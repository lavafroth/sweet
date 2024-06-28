use sweet::{
    token::{Key, KeyAttribute, Modifier},
    Binding, Definition, ParseError, ParserInput, SwhkdParser,
};

#[test]
fn test_basic_keybind() -> Result<(), ParseError> {
    let contents = "
r
    alacritty
            ";
    SwhkdParser::from(ParserInput::Raw(&contents))?;
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
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;

    let known = vec![
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key::new(evdev::Key::KEY_R, KeyAttribute::None),
            },
            command: "alacritty".to_string().to_string(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key::new(evdev::Key::KEY_W, KeyAttribute::None),
            },
            command: "kitty".to_string().to_string(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key::new(evdev::Key::KEY_T, KeyAttribute::None),
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
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;

    let known = vec![
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key::new(evdev::Key::KEY_R, KeyAttribute::None),
            },
            command: "alacritty".to_string().to_string(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![],
                key: Key::new(evdev::Key::KEY_W, KeyAttribute::None),
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

    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    let known = vec![Binding {
        definition: Definition {
            modifiers: vec![Modifier::Super],
            key: Key::new(evdev::Key::KEY_5, KeyAttribute::None),
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

    assert!(SwhkdParser::from(ParserInput::Raw(&contents)).is_err());
}

#[test]
fn test_modifier_instead_of_keysym() {
    let contents = "
shift + k + alt
    notify-send 'Hello world!'
            ";

    assert!(SwhkdParser::from(ParserInput::Raw(&contents)).is_err());
}

#[test]
fn test_unfinished_plus_sign() {
    let contents = "


shift + alt +
    notify-send 'Hello world!'
            ";
    assert!(SwhkdParser::from(ParserInput::Raw(&contents)).is_err());
}

#[test]
fn test_plus_sign_at_start() {
    let contents = "
+ shift + k
    notify-send 'Hello world!'
            ";

    assert!(SwhkdParser::from(ParserInput::Raw(&contents)).is_err());
}

#[test]
fn test_common_modifiers() -> Result<(), ParseError> {
    let contents = "
shift + k
    notify-send 'Hello world!'

control + 5
    notify-send 'Hello world!'

alt + 2
    notify-send 'Hello world!'

altgr + i
    notify-send 'Hello world!'

super + z
    notify-send 'Hello world!'
            ";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;

    let command = "notify-send 'Hello world!'".to_string();
    let known = vec![
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Shift],
                key: Key::new(evdev::Key::KEY_K, KeyAttribute::None),
            },
            command: command.clone(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Control],
                key: Key::new(evdev::Key::KEY_5, KeyAttribute::None),
            },
            command: command.clone(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Alt],
                key: Key::new(evdev::Key::KEY_2, KeyAttribute::None),
            },
            command: command.clone(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Altgr],
                key: Key::new(evdev::Key::KEY_I, KeyAttribute::None),
            },
            command: command.clone(),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Super],
                key: Key::new(evdev::Key::KEY_Z, KeyAttribute::None),
            },
            command: command.clone(),
        },
    ];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
fn test_command_with_many_spaces() -> Result<(), ParseError> {
    let contents = "
p
    xbacklight -inc 10 -fps 30 -time 200
        ";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;

    let known = vec![Binding {
        definition: Definition {
            modifiers: vec![],
            key: Key::new(evdev::Key::KEY_P, KeyAttribute::None),
        },
        command: String::from("xbacklight -inc 10 -fps 30 -time 200"),
    }];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
fn test_invalid_keybinding() {
    let contents = "
p
    xbacklight -inc 10 -fps 30 -time 200

pesto
    xterm
                    ";

    assert!(SwhkdParser::from(ParserInput::Raw(&contents)).is_err());
}

#[test]
// NOTE: This behavior is stricter than the older parser.
// Don't silently ignore keysyms not followed by command.
fn test_no_command() {
    let contents = "
k
    xbacklight -inc 10 -fps 30 -time 200

w

                    ";

    assert!(SwhkdParser::from(ParserInput::Raw(&contents)).is_err());
}

#[test]
fn test_real_config_snippet() -> Result<(), ParseError> {
    let contents = "
# reloads sxhkd configuration:
super + Escape
    pkill -USR1 -x sxhkd ; sxhkd &

# Launch Terminal
super + Return
    alacritty -t \"Terminal\" -e \"$HOME/.config/sxhkd/new_tmux_terminal.sh\"

# terminal emulator (no tmux)
super + shift + Return
    alacritty -t \"Terminal\"

# terminal emulator (new tmux session)
alt + Return
    alacritty -t \"Terminal\" -e \"tmux\"

ctrl + 0
    play-song.sh

super + minus
    play-song.sh album
                    ";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;

    let known = vec![
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Super],
                key: Key::new(evdev::Key::KEY_ESC, KeyAttribute::None),
            },
            command: String::from("pkill -USR1 -x sxhkd ; sxhkd &"),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Super],
                key: Key::new(evdev::Key::KEY_ENTER, KeyAttribute::None),
            },
            command: String::from(
                "alacritty -t \"Terminal\" -e \"$HOME/.config/sxhkd/new_tmux_terminal.sh\"",
            ),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Super, Modifier::Shift],
                key: Key::new(evdev::Key::KEY_ENTER, KeyAttribute::None),
            },
            command: String::from("alacritty -t \"Terminal\""),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Alt],
                key: Key::new(evdev::Key::KEY_ENTER, KeyAttribute::None),
            },
            command: String::from("alacritty -t \"Terminal\" -e \"tmux\""),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Control],
                key: Key::new(evdev::Key::KEY_0, KeyAttribute::None),
            },
            command: String::from("play-song.sh"),
        },
        Binding {
            definition: Definition {
                modifiers: vec![Modifier::Super],
                key: Key::new(evdev::Key::KEY_MINUS, KeyAttribute::None),
            },
            command: String::from("play-song.sh album"),
        },
    ];

    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_multiline_command() -> Result<(), ParseError> {
    let contents = "
k
    mpc ls | dmenu | \\
    sed -i 's/foo/bar/g'
                    ";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;

    let known = vec![Binding {
        definition: Definition {
            modifiers: vec![],
            key: Key::new(evdev::Key::KEY_K, KeyAttribute::None),
        },
        command: String::from("mpc ls | dmenu | sed -i 's/foo/bar/g'"),
    }];

    assert_eq!(parsed.bindings, known);
    Ok(())
}
