use sweet::{
    token::{Key, KeyAttribute, Modifier},
    Binding, Definition, ParseError, SwhkdParser,
};

#[test]
fn test_basic_keybind() -> Result<(), ParseError> {
    let contents = "
r
    alacritty
            ";
    SwhkdParser::from(&contents)?;
    Ok(())
}

#[test]
fn test_multiple_keybinds() -> Result<(), ParseError> {
    let contents = "
r
    alacritty

w
    kitty

t
    /bin/firefox
        ";
    let parsed = SwhkdParser::from(&contents)?;

    let known = vec![
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key {
                    key: "r".to_string(),
                    attribute: KeyAttribute::None,
                },
            },
            command: "alacritty".to_string().to_string(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key {
                    key: "w".to_string(),
                    attribute: KeyAttribute::None,
                },
            },
            command: "kitty".to_string().to_string(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key {
                    key: "t".to_string(),
                    attribute: KeyAttribute::None,
                },
            },
            command: "/bin/firefox".to_string().to_string(),
        },
    ];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
fn test_comments() -> Result<(), ParseError> {
    let contents = "
r
    alacritty

w
    kitty

#t
    #/bin/firefox
        ";
    let parsed = SwhkdParser::from(&contents)?;

    let known = vec![
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key {
                    key: "r".to_string(),
                    attribute: KeyAttribute::None,
                },
            },
            command: "alacritty".to_string().to_string(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key {
                    key: "w".to_string(),
                    attribute: KeyAttribute::None,
                },
            },
            command: "kitty".to_string().to_string(),
        },
    ];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
fn test_multiple_keypress() -> Result<(), ParseError> {
    let contents = "
super + 5
    alacritty
        ";

    let parsed = SwhkdParser::from(&contents)?;
    let known = vec![Binding {
        definition: Definition {
            modifiers: vec![Modifier("super".to_string())],
            key: Key {
                key: "5".to_string(),
                attribute: KeyAttribute::None,
            },
        },
        command: "alacritty".to_string().to_string(),
    }];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
fn test_keysym_instead_of_modifier() {
    let contents = "
shift + k + m
    notify-send 'Hello world!'
            ";

    assert!(SwhkdParser::from(&contents).is_err());
}