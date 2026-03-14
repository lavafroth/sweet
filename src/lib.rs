use itertools::Itertools;
use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use range::Bounds;
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use thiserror::Error;

mod bindings;
mod definition;
mod evdev_mappings;
mod range;
mod token;

pub use crate::bindings::Binding;
pub use crate::definition::{Definition, DefinitionUncompiled};
pub use crate::token::{Key, KeyAttribute, KeyRepr, Modifier, ModifierRepr};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unable to parse config file")]
    // pest::error::Error being 184 bytes makes this entire enum
    // expensive to copy, hence the box is used to put it on the heap.
    Grammar(#[from] Box<pest::error::Error<Rule>>),
    #[error("hotkey config must contain one and only one main section")]
    MainSection,
    #[error(transparent)]
    ConfigRead(#[from] ConfigReadError),
    #[error("`{0}` is not recongnized as a valid evdev key")]
    InvalidKey(String),
}

#[derive(Parser)]
#[grammar = "template.pest"]
pub struct SwhkdGrammar;

#[derive(Default, Debug)]
pub struct Mode {
    pub name: String,
    pub oneoff: bool,
    pub swallow: bool,
    pub bindings: Vec<Binding>,
    pub unbinds: Vec<Definition>,
}

#[derive(Debug)]
pub struct SwhkdParser {
    pub bindings: Vec<Binding>,
    pub unbinds: Vec<Definition>,
    pub imports: BTreeSet<String>,
    pub modes: Vec<Mode>,
}

/// Input to the grammar parser.
/// Can be either a string or a path.
pub enum ParserInput<'a> {
    Raw(&'a str),
    Path(&'a Path),
}

#[derive(Debug, Error)]
pub enum ConfigReadError {
    #[error("unable to read config file")]
    ReadingConfig(#[from] std::io::Error),
    #[error("path `{0}` supplied as config is not a regular file")]
    NotRegularFile(PathBuf),
    #[error("the supplied config file {0} size exceeds the {1}MiB limit")]
    TooLarge(PathBuf, u64),
}

pub fn read_config<P: AsRef<Path>>(path: P) -> Result<String, ConfigReadError> {
    let path = path.as_ref();
    let stat = fs::metadata(path)?;
    if !stat.is_file() {
        return Err(ConfigReadError::NotRegularFile(path.to_path_buf()));
    }
    let size = stat.size();
    let mib_cap = std::option_env!("FILESIZE_CAP_MIB")
        .and_then(|cap| cap.parse().ok())
        .unwrap_or(50);
    if size > (mib_cap << 20) {
        return Err(ConfigReadError::TooLarge(path.to_path_buf(), mib_cap));
    }
    // TODO: Use mmap instead of fs::read_to_string
    Ok(fs::read_to_string(path)?)
}

impl SwhkdParser {
    pub fn from(input: ParserInput) -> Result<Self, ParseError> {
        let mut imports = BTreeSet::new();
        let root = Self::as_import(input, &mut imports)?;
        let mut bindings: Vec<Binding> = vec![];
        for binding in root.bindings {
            if let Some(b) = bindings
                .iter_mut()
                .find(|b| b.definition == binding.definition)
            {
                b.command = binding.command;
                b.mode_instructions = binding.mode_instructions;
                continue;
            }

            if root
                .unbinds
                .iter()
                .find(|b| binding.definition.eq(b))
                .is_some()
            {
                continue;
            }
            bindings.push(binding);
        }
        Ok(SwhkdParser {
            bindings,
            imports,
            unbinds: root.unbinds,
            modes: root.modes,
        })
    }
    fn as_import(input: ParserInput, seen: &mut BTreeSet<String>) -> Result<Self, ParseError> {
        let (raw, source) = match input {
            // If a config is loaded from a string instead of a path, name it `<anonymous>`
            ParserInput::Raw(s) => (s.to_string(), "<anonymous>"),
            ParserInput::Path(p) => (read_config(p)?, p.to_str().unwrap_or_default()),
        };
        let parse_result = SwhkdGrammar::parse(Rule::main, &raw)
            .map_err(|err| ParseError::Grammar(Box::new(err.with_path(source))))?;

        let Some(contents) = parse_result.into_iter().next() else {
            return Err(ParseError::MainSection);
        };

        let mut bindings: Vec<Binding> = vec![];
        let mut unbinds = vec![];
        let mut imports = BTreeSet::new();
        let mut modes = vec![];
        for decl in contents.into_inner() {
            match decl.as_rule() {
                Rule::binding => bindings.extend(binding_parser(decl)?),
                Rule::unbind => unbinds.extend(unbind_parser(decl)?),
                Rule::mode => modes.push(mode_parser(decl)?),
                Rule::import => imports.extend(import_parser(decl)),
                // End of identifier
                // Here, it means the end of the file.
                Rule::EOI => {}
                _ => unreachable!(),
            }
        }

        while let Some(import) = imports.pop_first() {
            if !seen.insert(import.clone()) {
                continue;
            }
            let child = Self::as_import(ParserInput::Path(Path::new(&import)), seen)?;
            bindings.extend(child.bindings);
            imports.extend(child.imports);
            unbinds.extend(child.unbinds);
            modes.extend(child.modes);
        }
        Ok(SwhkdParser {
            bindings,
            unbinds,
            imports,
            modes,
        })
    }
}

fn pair_to_string(pair: Pair<'_, Rule>) -> String {
    pair.as_str().to_string()
}

/// Unescapes a string that has been escaped using backslashes
/// but only for the charset of '{}|\-+~@,' that were allowed to
/// be escaped in the grammar in the first place.
/// ```ignore
/// use sweet::unescape;
/// fn main() {
/// assert_eq!(unescape(r"hello\\\{\}\|\-\+\~\@\,"), r"hello\{}|-+~@,");
/// }    
/// ```
fn unescape(s: &str) -> String {
    let mut unescaped = String::with_capacity(s.len());
    let mut was_a_slash = None;
    for char in s.chars() {
        match (char, was_a_slash.take()) {
            ('\\', None) => was_a_slash = Some(()),
            ('\\', Some(())) => unescaped.push(char),
            ('{' | '}' | '|' | '-' | '+' | '~' | '@' | ',', Some(())) => unescaped.push(char),
            (_, Some(())) => {}
            _ => unescaped.push(char),
        }
    }
    unescaped
}
fn unbind_parser(pair: Pair<'_, Rule>) -> Result<Vec<Definition>, ParseError> {
    let mut uncompiled = DefinitionUncompiled::default();
    for thing in pair.into_inner() {
        uncompiled.ingest(thing)?;
    }
    Ok(uncompiled.compile())
}

fn import_parser(pair: Pair<'_, Rule>) -> Vec<String> {
    pair.into_inner()
        .filter(|component| matches!(component.as_rule(), Rule::import_file))
        .map(pair_to_string)
        .collect()
}

fn parse_key(component: Pair<'_, Rule>) -> KeyRepr {
    let mut attribute = KeyAttribute::None;
    let mut key = String::default();
    for inner in component.into_inner() {
        match inner.as_rule() {
            Rule::send => attribute |= KeyAttribute::Send,
            Rule::on_release => attribute |= KeyAttribute::OnRelease,
            Rule::shorthand_allow | Rule::key_base => {
                key = unescape(&inner.as_str().to_lowercase()).to_string()
            }
            _ => {}
        }
    }
    KeyRepr { key, attribute }
}

fn parse_command_shorthand(pair: Pair<'_, Rule>) -> Result<Vec<String>, ParseError> {
    let mut command_variants = vec![];

    for component in pair.into_inner() {
        match component.as_rule() {
            Rule::command_component => {
                command_variants.push(unescape(component.as_str()).to_string())
            }
            Rule::range => {
                let (lower_bound, upper_bound) = Bounds::new(component).expand_commands()?;
                command_variants.extend((lower_bound..=upper_bound).map(|key| key.to_string()));
            }
            _ => {}
        }
    }
    Ok(command_variants)
}
fn mode_parser(pair: Pair<'_, Rule>) -> Result<Mode, ParseError> {
    let mut mode = Mode::default();
    for component in pair.into_inner() {
        match component.as_rule() {
            Rule::modename => mode.name = component.as_str().to_string(),
            Rule::binding => mode.bindings.extend(binding_parser(component)?),
            Rule::unbind => mode.unbinds.extend(unbind_parser(component)?),
            Rule::oneoff => mode.oneoff = true,
            Rule::swallow => mode.swallow = true,
            _ => {}
        }
    }
    Ok(mode)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeInstruction {
    Enter(String),
    Escape,
}

fn binding_parser(pair: Pair<'_, Rule>) -> Result<Vec<Binding>, ParseError> {
    let mut comm = vec![];
    let mut mode_enters = vec![];
    let mut mode_escapes = vec![];
    let mut uncompiled = DefinitionUncompiled::default();
    for component in pair.clone().into_inner() {
        match component.as_rule() {
            Rule::command => {
                for subcomponent in component.into_inner() {
                    match subcomponent.as_rule() {
                        Rule::command_standalone => {
                            comm.push(vec![pair_to_string(subcomponent)]);
                        }
                        Rule::command_shorthand => {
                            comm.push(parse_command_shorthand(subcomponent)?);
                        }
                        Rule::command_double_ampersand => {
                            if comm
                                .last()
                                .is_some_and(|last| last.len() == 1 && last[0] == "&&")
                            {
                                continue;
                            }
                            comm.push(vec![pair_to_string(subcomponent)]);
                        }
                        Rule::enter_mode => {
                            // Safety: the first element is guaranteed to be a modename
                            // by the grammar.
                            let modename = subcomponent.into_inner().next().unwrap();
                            mode_enters.push(ModeInstruction::Enter(pair_to_string(modename)));
                        }
                        Rule::escape_mode => {
                            if mode_enters.pop().is_none() {
                                mode_escapes.push(ModeInstruction::Escape);
                            }
                        }
                        _ => {}
                    }
                }

                if comm
                    .last()
                    .is_some_and(|last| last.len() == 1 && last[0] == "&&")
                {
                    comm.pop();
                }
            }
            _ => uncompiled.ingest(component)?,
        }
    }
    let bind_cartesian_product = uncompiled.compile();
    let command_cartesian_product = comm
        .into_iter()
        .multi_cartesian_product()
        .map(|c| c.join(""))
        .collect_vec();
    let bind_len = bind_cartesian_product.len();
    let command_len = command_cartesian_product.len();
    let instructions_len = mode_enters.len() + mode_escapes.len();

    if bind_len != command_len && instructions_len == 0 {
        let err = pest::error::Error::new_from_span(
            pest::error::ErrorVariant::<Rule>::CustomError {
                message: format!(
                    "the number of possible binding variants {0} does not equal the number of possible command variants {1}.",
                    bind_len, command_len
                ),
            },
            pair.as_span(),
        );
        return Err(Box::new(err).into());
    }

    let mut bindings: Vec<Binding> = bind_cartesian_product
        .into_iter()
        .zip(command_cartesian_product)
        .map(|(definition, command)| Binding {
            definition,
            command,
            mode_instructions: mode_enters
                .iter()
                .chain(mode_escapes.iter())
                .cloned()
                .collect(),
        })
        .collect();

    for binding in bindings.iter_mut() {
        binding.definition.modifiers.remove(&Modifier::Omission);
    }
    Ok(bindings)
}
