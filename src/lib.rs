use itertools::Itertools;
use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use range::Bounds;
use std::fmt::Display;
use thiserror::Error;
mod range;
mod token;
use crate::token::{Key, KeyAttribute, Modifier};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unable to parse grammar from invalid contents")]
    // pest::error::Error being 184 bytes makes this entire enum
    // expensive to copy, hence the box is used to put it on the heap.
    Grammar(#[from] Box<pest::error::Error<Rule>>),
    #[error("hotkey config must contain one and only one main section")]
    MainSection,
}

#[derive(Parser)]
#[grammar = "template.pest"]
pub struct SwhkdGrammar;

pub struct SwhkdParser {
    pub bindings: Vec<Binding>,
    pub unbinds: Vec<Definition>,
    pub imports: Vec<String>,
}

impl SwhkdParser {
    pub fn from(raw: &str) -> Result<Self, ParseError> {
        let parse_result = SwhkdGrammar::parse(Rule::main, raw)
            .map_err(|err| ParseError::Grammar(Box::new(err)))?;
        let Some(contents) = parse_result.into_iter().next() else {
            return Err(ParseError::MainSection);
        };

        let mut bindings = vec![];
        let mut unbinds = vec![];
        let mut imports = vec![];
        for decl in contents.into_inner() {
            match decl.as_rule() {
                Rule::binding => bindings.extend(binding_parser(decl)?),
                Rule::unbind => unbinds.extend(unbind_parser(decl)?),
                Rule::import => imports.extend(import_parser(decl)),
                // End of identifier
                // Here, it means the end of the file.
                Rule::EOI => {}
                _ => unreachable!(),
            }
        }
        Ok(SwhkdParser {
            bindings,
            unbinds,
            imports,
        })
    }
}

#[derive(Debug)]
pub struct Definition {
    modifiers: Vec<Modifier>,
    key: Key,
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

#[derive(Debug)]
pub struct Binding {
    pub definition: Definition,
    pub command: String,
}
impl Display for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Binding {} \u{2192} {}", self.definition, self.command)
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

fn parse_key(component: Pair<'_, Rule>) -> Key {
    let mut attribute = KeyAttribute::None;
    let mut key = String::default();
    for inner in component.into_inner() {
        match inner.as_rule() {
            Rule::send => attribute |= KeyAttribute::Send,
            Rule::on_release => attribute |= KeyAttribute::OnRelease,
            Rule::shorthand_allow | Rule::key_base => key = unescape(inner.as_str()).to_string(),
            _ => {}
        }
    }
    Key { key, attribute }
}

#[derive(Default)]
pub struct DefinitionUncompiled {
    pub modifiers: Vec<Vec<Modifier>>,
    pub keys: Vec<Key>,
}

impl DefinitionUncompiled {
    fn ingest(&mut self, component: Pair<'_, Rule>) -> Result<(), ParseError> {
        match component.as_rule() {
            Rule::modifier => self
                .modifiers
                .push(vec![Modifier(pair_to_string(component))]),
            Rule::modifier_shorthand | Rule::modifier_omit_shorthand => self.modifiers.push(
                component
                    .into_inner()
                    .map(|component| Modifier(pair_to_string(component)))
                    .collect(),
            ),
            Rule::shorthand => {
                for shorthand_component in component.into_inner() {
                    match shorthand_component.as_rule() {
                        Rule::key_in_shorthand => self.keys.push(parse_key(shorthand_component)),
                        Rule::key_range => {
                            let (lower_bound, upper_bound) =
                                Bounds::new(shorthand_component).expand_keys()?;
                            self.keys.extend((lower_bound..=upper_bound).map(|key| Key {
                                key: key.to_string(),
                                attribute: KeyAttribute::None,
                            }));
                        }
                        _ => {}
                    }
                }
            }
            Rule::key_normal => self.keys.push(parse_key(component)),
            _ => {}
        };
        Ok(())
    }

    fn compile(self) -> Vec<Definition> {
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

fn binding_parser(pair: Pair<'_, Rule>) -> Result<Vec<Binding>, ParseError> {
    let mut comm = vec![];
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
        .map(|c| c.join(" "))
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
        })
        .collect();
    Ok(bindings)
}
