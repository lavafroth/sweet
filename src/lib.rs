use itertools::Itertools;
use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use range::Bounds;
use std::{
    collections::BTreeSet,
    fmt::Display,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use thiserror::Error;
use token::{KeyRepr, Modifier};
mod evdev_mappings;
mod range;
pub mod token;
use crate::token::{Key, KeyAttribute, ModifierRepr};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unable to parse grammar from invalid contents")]
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
    #[error("the supplied config file {0} size exceeds the 50MiB limit")]
    TooLarge(PathBuf),
}

pub fn read_config<P: AsRef<Path>>(path: P) -> Result<String, ConfigReadError> {
    let path = path.as_ref();
    let stat = fs::metadata(path)?;
    if !stat.is_file() {
        return Err(ConfigReadError::NotRegularFile(path.to_path_buf()));
    }
    let size = stat.size();
    /* 50MiB size cap */
    if size > (50 << 20) {
        return Err(ConfigReadError::TooLarge(path.to_path_buf()));
    }
    // TODO: Use mmap instead of fs::read_to_string
    Ok(fs::read_to_string(path)?)
}

impl SwhkdParser {
    pub fn from(input: ParserInput) -> Result<Self, ParseError> {
        let mut root_imports = BTreeSet::new();
        let mut root = Self::as_import(input, &mut root_imports)?;
        root.imports = root_imports;
        Ok(root)
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

        let mut bindings = vec![];
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
            if seen.contains(&import) {
                continue;
            }
            seen.insert(import.clone());
            let child = Self::as_import(ParserInput::Path(Path::new(&import)), seen)?;
            imports.extend(child.imports);
            bindings.extend(child.bindings);
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

#[derive(Debug, PartialEq, Eq)]
pub struct Definition {
    pub modifiers: Vec<Modifier>,
    pub key: Key,
}

impl Display for Definition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for modifier in self.modifiers.iter() {
            write!(f, "{:?}, ", modifier)?;
        }
        write!(f, "{:?}", self.key)?;
        write!(f, "]")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Binding {
    pub definition: Definition,
    pub command: String,
    pub mode_instructions: Vec<ModeInstruction>,
}
impl Display for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Binding {} \u{2192} {} (mode instructions: {:?})",
            self.definition, self.command, self.mode_instructions
        )
    }
}

fn pair_to_string(pair: Pair<'_, Rule>) -> String {
    pair.as_str().to_string()
}

fn unescape(s: &str) -> &str {
    let chars: Vec<_> = s.chars().collect();
    let ['\\', ch] = &chars[..] else {
        return s;
    };
    // Pest guarantees this for us. Still keeping a bit of sanity check.
    assert!(matches!(ch, '{' | '}' | ',' | '\\' | '-' | '+' | '~' | '@'));
    &s[1..]
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

#[derive(Default)]
pub struct DefinitionUncompiled {
    pub modifiers: Vec<Vec<Modifier>>,
    pub keys: Vec<Key>,
}

impl DefinitionUncompiled {
    fn ingest(&mut self, component: Pair<'_, Rule>) -> Result<(), ParseError> {
        match component.as_rule() {
            Rule::modifier => {
                self.modifiers.push(vec![
                    ModifierRepr(pair_to_string(component).to_lowercase()).into()
                ])
            }
            Rule::modifier_shorthand | Rule::modifier_omit_shorthand => self.modifiers.push(
                component
                    .into_inner()
                    .map(|component| ModifierRepr(pair_to_string(component)).into())
                    .collect(),
            ),
            Rule::shorthand => {
                for shorthand_component in component.into_inner() {
                    match shorthand_component.as_rule() {
                        Rule::key_in_shorthand => {
                            self.keys.push(parse_key(shorthand_component).try_into()?)
                        }
                        Rule::key_range => {
                            let (lower_bound, upper_bound) =
                                Bounds::new(shorthand_component).expand_keys()?;
                            let keys = (lower_bound..=upper_bound)
                                .map(|key| {
                                    KeyRepr {
                                        key: key.to_string(),
                                        attribute: KeyAttribute::None,
                                    }
                                    .try_into()
                                })
                                .collect::<Result<Vec<Key>, ParseError>>()?;
                            self.keys.extend(keys);
                        }
                        _ => {}
                    }
                }
            }
            Rule::key_normal => self.keys.push(parse_key(component).try_into()?),
            _ => {}
        };
        Ok(())
    }

    fn compile(self) -> Vec<Definition> {
        if self.modifiers.is_empty() {
            return self
                .keys
                .into_iter()
                .map(|key| Definition {
                    modifiers: vec![],
                    key,
                })
                .collect();
        }
        self.modifiers
            .into_iter()
            .multi_cartesian_product()
            .cartesian_product(self.keys)
            .map(|(modifiers, key)| Definition { modifiers, key })
            .collect()
    }
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

fn parse_command_shorthand(pair: Pair<'_, Rule>) -> Result<Vec<String>, ParseError> {
    let mut command_variants = vec![];

    for component in pair.into_inner() {
        match component.as_rule() {
            Rule::command_component => command_variants.push(pair_to_string(component)),
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

    if bind_len != command_len {
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

    let bindings = bind_cartesian_product
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
    Ok(bindings)
}
