use std::io::Write;

use pest::error::LineColLocation::{Pos, Span};
use sweet::{
    Binding, Definition, ParseError, ParserInput, SwhkdParser,
    {Key, KeyAttribute, Modifier::*},
};
use thiserror::Error;

fn assert_grammar_error_at(contents: &str, pos: (usize, usize)) {
    let parse_result = SwhkdParser::from(ParserInput::Raw(contents));
    let Err(ParseError::Grammar(e)) = parse_result else {
        panic!("expected grammar parse error")
    };
    assert_eq!(e.line_col, Pos(pos));
}
fn assert_grammar_error_at_span(contents: &str, start: (usize, usize), end: (usize, usize)) {
    let parse_result = SwhkdParser::from(ParserInput::Raw(contents));
    let Err(ParseError::Grammar(e)) = parse_result else {
        panic!("expected grammar parse error")
    };
    assert_eq!(e.line_col, Span(start, end));
}

fn assert_equal_binding_set(a: Vec<Binding>, mut b: Vec<Binding>) {
    for binding in a {
        if let Some(pos) = b.iter().position(|bin| bin.eq(&binding)) {
            b.remove(pos);
        }
    }
    assert_eq!(b, vec![]);
}

#[test]
fn test_existing_file() -> std::io::Result<()> {
    let mut setup = tempfile::NamedTempFile::new()?;
    let setup_path = setup.path().to_owned();
    setup.write_all(
        b"
x
    dmenu_run

q
    bspc node -q",
    )?;
    // setup gets dropped here
    std::fs::read_to_string(setup_path)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum IoOrParseError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

#[test]
fn test_load_multiple_config() -> Result<(), IoOrParseError> {
    let mut import = tempfile::NamedTempFile::new()?;
    import.write_all(
        b"
super + c
    hello",
    )?;

    let mut setup = tempfile::NamedTempFile::new()?;
    write!(
        setup,
        "
include {}
super + b
   firefox",
        import.path().display()
    )?;

    let parsed = SwhkdParser::from(ParserInput::Path(setup.path()))?;
    let known = [
        Binding::running("firefox").on(Definition::new(evdev::Key::KEY_B).with_modifiers(&[Super])),
        Binding::running("hello").on(Definition::new(evdev::Key::KEY_C).with_modifiers(&[Super])),
    ];
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_relative_import() -> Result<(), IoOrParseError> {
    // create a temporary file in the working directory
    // this gets cleaned on a `drop` call
    let mut setup = tempfile::NamedTempFile::new_in(".")?;
    setup.write_all(
        b"
super + c
    hello",
    )?;
    let mut import = tempfile::NamedTempFile::new()?;

    write!(
        import,
        "
include {}
super + b
   firefox",
        setup.path().display()
    )?;

    let parsed = SwhkdParser::from(ParserInput::Path(import.path()))?;
    let known = [
        Binding::running("firefox").on(Definition::new(evdev::Key::KEY_B).with_modifiers(&[Super])),
        Binding::running("hello").on(Definition::new(evdev::Key::KEY_C).with_modifiers(&[Super])),
    ];
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_circular_import() -> Result<(), IoOrParseError> {
    let mut setup = tempfile::NamedTempFile::new()?;
    setup.write_all(
        b"
a
    a",
    )?;

    let mut setup2 = tempfile::NamedTempFile::new()?;
    write!(
        setup2,
        "
include {}
b
    b",
        setup.path().display()
    )?;
    let mut setup3 = tempfile::NamedTempFile::new()?;
    let setup3_path = setup3.path().to_owned();
    let mut setup4 = tempfile::NamedTempFile::new()?;
    write!(
        setup3,
        "
include {}
include {}
include {}
include {}
c
    c",
        setup.path().display(),
        setup2.path().display(),
        setup3_path.display(),
        setup4.path().display(),
    )?;
    write!(
        setup4,
        "
include {}
d
    d",
        setup3.path().display()
    )?;
    let parsed = SwhkdParser::from(ParserInput::Path(setup4.path()))?;
    let known = vec![
        Binding::running("d").on(Definition::new(evdev::Key::KEY_D)),
        Binding::running("c").on(Definition::new(evdev::Key::KEY_C)),
        Binding::running("b").on(Definition::new(evdev::Key::KEY_B)),
        Binding::running("a").on(Definition::new(evdev::Key::KEY_A)),
    ];
    assert_equal_binding_set(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_include_and_unbind() -> Result<(), IoOrParseError> {
    let mut setup = tempfile::NamedTempFile::new()?;
    let mut setup2 = tempfile::NamedTempFile::new()?;
    setup2.write_all(
        b"
super + c
    hello
super + d
    world",
    )?;

    write!(
        setup,
        "
include {}
super + b
   firefox
ignore super + d",
        setup2.path().display()
    )?;

    let parsed = SwhkdParser::from(ParserInput::Path(setup.path()))?;
    let known = [
        Binding::running("firefox").on(Definition::new(evdev::Key::KEY_B).with_modifiers(&[Super])),
        Binding::running("hello").on(Definition::new(evdev::Key::KEY_C).with_modifiers(&[Super])),
    ];
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_basic_keybind() -> Result<(), ParseError> {
    let contents = "
r
    alacritty
            ";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    let known = [Binding::running("alacritty").on(Definition::new(evdev::Key::KEY_R))];

    assert_eq!(parsed.bindings, known);
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

    let known = [
        Binding::running("alacritty").on(Definition::new(evdev::Key::KEY_R)),
        Binding::running("kitty").on(Definition::new(evdev::Key::KEY_W)),
        Binding::running("/bin/firefox").on(Definition::new(evdev::Key::KEY_T)),
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
        Binding::running("alacritty").on(Definition::new(evdev::Key::KEY_R)),
        Binding::running("kitty").on(Definition::new(evdev::Key::KEY_W)),
    ];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
fn test_greedy_ordering() -> Result<(), ParseError> {
    let contents = "
f9
    librewolf

f12
    kitty
        ";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;

    let known = vec![
        Binding::running("librewolf").on(Definition::new(evdev::Key::KEY_F9)),
        Binding::running("kitty").on(Definition::new(evdev::Key::KEY_F12)),
    ];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
// this test is stricter than the previous parser
// gimp is not a valid key or a modifier
fn test_commented_out_keybind() {
    let contents = "
#w
    gimp";
    assert_grammar_error_at(&contents, (3, 6));
}

#[test]
fn test_inline_comment() -> Result<(), IoOrParseError> {
    let contents = "
super + a #comment and comment super
    st
super + shift + b
    ts #this comment should be handled by shell
";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    let known = [
        Binding::running("st").on(Definition::new(evdev::Key::KEY_A).with_modifiers(&[Super])),
        Binding::running("ts #this comment should be handled by shell")
            .on(Definition::new(evdev::Key::KEY_B).with_modifiers(&[Super, Shift])),
    ];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
fn test_blank_config() -> Result<(), ParseError> {
    let contents = "";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, vec![]);
    Ok(())
}

#[test]
fn test_blank_config_with_whitespace() -> Result<(), ParseError> {
    let contents = "


            ";
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, []);
    Ok(())
}

#[test]
fn test_multiple_keypress() -> Result<(), ParseError> {
    let contents = "
super + 5
    alacritty
        ";

    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    let known = [Binding::running("alacritty")
        .on(Definition::new(evdev::Key::KEY_5).with_modifiers(&[Super]))];

    assert_eq!(parsed.bindings, known);

    Ok(())
}

#[test]
fn test_keysym_instead_of_modifier() {
    let contents = "
shift + k + m
    notify-send 'Hello world!'
            ";

    let parse_result = SwhkdParser::from(ParserInput::Raw(&contents));
    let Err(ParseError::Grammar(e)) = parse_result else {
        panic!("expected grammar parse error")
    };
    assert_eq!(e.line_col, Pos((2, 11)));
}

#[test]
fn test_modifier_instead_of_keysym() {
    let contents = "
shift + k + alt
    notify-send 'Hello world!'
            ";
    assert_grammar_error_at(&contents, (2, 11));
}

#[test]
fn test_unfinished_plus_sign() {
    let contents = "


shift + alt +
    notify-send 'Hello world!'
            ";
    assert_grammar_error_at(&contents, (4, 14));
}

#[test]
fn test_plus_sign_at_start() {
    let contents = "
+ shift + k
    notify-send 'Hello world!'
            ";
    assert_grammar_error_at(&contents, (2, 1));
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

    let command = "notify-send 'Hello world!'";
    let known = [
        Binding::running(command).on(Definition::new(evdev::Key::KEY_K).with_modifiers(&[Shift])),
        Binding::running(command).on(Definition::new(evdev::Key::KEY_5).with_modifiers(&[Control])),
        Binding::running(command).on(Definition::new(evdev::Key::KEY_2).with_modifiers(&[Alt])),
        Binding::running(command).on(Definition::new(evdev::Key::KEY_I).with_modifiers(&[Altgr])),
        Binding::running(command).on(Definition::new(evdev::Key::KEY_Z).with_modifiers(&[Super])),
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
    let known = [Binding::running("xbacklight -inc 10 -fps 30 -time 200")
        .on(Definition::new(evdev::Key::KEY_P))];

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
    assert_grammar_error_at(&contents, (5, 2));
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
        Binding::running("pkill -USR1 -x sxhkd ; sxhkd &")
            .on(Definition::new(evdev::Key::KEY_ESC).with_modifiers(&[Super])),
        Binding::running(
            "alacritty -t \"Terminal\" -e \"$HOME/.config/sxhkd/new_tmux_terminal.sh\"",
        )
        .on(Definition::new(evdev::Key::KEY_ENTER).with_modifiers(&[Super])),
        Binding::running("alacritty -t \"Terminal\"")
            .on(Definition::new(evdev::Key::KEY_ENTER).with_modifiers(&[Super, Shift])),
        Binding::running("alacritty -t \"Terminal\" -e \"tmux\"")
            .on(Definition::new(evdev::Key::KEY_ENTER).with_modifiers(&[Alt])),
        Binding::running("play-song.sh")
            .on(Definition::new(evdev::Key::KEY_0).with_modifiers(&[Control])),
        Binding::running("play-song.sh album")
            .on(Definition::new(evdev::Key::KEY_MINUS).with_modifiers(&[Super])),
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

    let known = [Binding::running("mpc ls | dmenu | sed -i 's/foo/bar/g'")
        .on(Definition::new(evdev::Key::KEY_K))];

    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_case_insensitive() -> Result<(), ParseError> {
    let contents = "
Super + SHIFT + alt + a
    st
ReTurn
    ts
            ";
    let known = vec![
        Binding::running("st")
            .on(Definition::new(evdev::Key::KEY_A).with_modifiers(&[Super, Shift, Alt])),
        Binding::running("ts").on(Definition::new(evdev::Key::KEY_ENTER)),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_override() -> Result<(), ParseError> {
    let contents = "
super + a
    1
super + a
    2";
    let known =
        vec![Binding::running("2").on(Definition::new(evdev::Key::KEY_A).with_modifiers(&[Super]))];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_any_modifier() -> Result<(), ParseError> {
    let contents = "
any + a
    1";
    let known =
        vec![Binding::running("1").on(Definition::new(evdev::Key::KEY_A).with_modifiers(&[Any]))];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_duplicate_hotkeys() -> Result<(), ParseError> {
    let contents = "
super + shift + a
    st
shift + suPer +   A
    ts
b
    st
B
    ts
";
    let known = [
        Binding::running("ts")
            .on(Definition::new(evdev::Key::KEY_A).with_modifiers(&[Super, Shift])),
        Binding::running("ts").on(Definition::new(evdev::Key::KEY_B)),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_range_syntax_ascii_character() -> Result<(), ParseError> {
    let contents = "
super + {a-c}
    {firefox, brave, librewolf}
    ";
    let known = vec![
        Binding::running("firefox").on(Definition::new(evdev::Key::KEY_A).with_modifiers(&[Super])),
        Binding::running("brave").on(Definition::new(evdev::Key::KEY_B).with_modifiers(&[Super])),
        Binding::running("librewolf")
            .on(Definition::new(evdev::Key::KEY_C).with_modifiers(&[Super])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_range_syntax_not_ascii() {
    let contents = "
super + {a-是}
    {firefox, brave}
    ";
    assert_grammar_error_at(contents, (2, 12));
}

#[test]
// This test differs from the previous iteration,
// dashes in shorthands will always mean ranges.
// Thus, they must be escaped.
fn test_multiple_shorthands() -> Result<(), ParseError> {
    let contents = r"
super + {shift,alt} + {c,d}
    {librewolf, firefox} {\-\-sync, \-\-help}
            ";
    let known = vec![
        Binding::running("librewolf --sync")
            .on(Definition::new(evdev::Key::KEY_C).with_modifiers(&[Super, Shift])),
        Binding::running("librewolf --help")
            .on(Definition::new(evdev::Key::KEY_D).with_modifiers(&[Super, Shift])),
        Binding::running("firefox --sync")
            .on(Definition::new(evdev::Key::KEY_C).with_modifiers(&[Super, Alt])),
        Binding::running("firefox --help")
            .on(Definition::new(evdev::Key::KEY_D).with_modifiers(&[Super, Alt])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
// This test differs from the previous iteration,
// dashes in shorthands will always mean ranges.
// Thus, they must be escaped.
fn test_multiple_ranges() -> Result<(), ParseError> {
    let contents = r"
{control,super} + {1-3}
    {notify\-send, echo} {hello,how,are}
            ";

    let known = vec![
        Binding::running("notify-send hello")
            .on(Definition::new(evdev::Key::KEY_1).with_modifiers(&[Control])),
        Binding::running("notify-send how")
            .on(Definition::new(evdev::Key::KEY_2).with_modifiers(&[Control])),
        Binding::running("notify-send are")
            .on(Definition::new(evdev::Key::KEY_3).with_modifiers(&[Control])),
        Binding::running("echo hello")
            .on(Definition::new(evdev::Key::KEY_1).with_modifiers(&[Super])),
        Binding::running("echo how")
            .on(Definition::new(evdev::Key::KEY_2).with_modifiers(&[Super])),
        Binding::running("echo are")
            .on(Definition::new(evdev::Key::KEY_3).with_modifiers(&[Super])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_valid_curly_brace_config() -> Result<(), ParseError> {
    let contents = "
super + {a,b,c}
    {firefox, brave, librewolf}
    ";
    let known = vec![
        Binding::running("firefox").on(Definition::new(evdev::Key::KEY_A).with_modifiers(&[Super])),
        Binding::running("brave").on(Definition::new(evdev::Key::KEY_B).with_modifiers(&[Super])),
        Binding::running("librewolf")
            .on(Definition::new(evdev::Key::KEY_C).with_modifiers(&[Super])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_valid_bspwm_curly_brace_config() -> Result<(), ParseError> {
    let contents = "
super + {h,j,k,l}
    bspc node -p {west, south, north, east}
    ";
    let known = [
        Binding::running("bspc node -p west")
            .on(Definition::new(evdev::Key::KEY_H).with_modifiers(&[Super])),
        Binding::running("bspc node -p south")
            .on(Definition::new(evdev::Key::KEY_J).with_modifiers(&[Super])),
        Binding::running("bspc node -p north")
            .on(Definition::new(evdev::Key::KEY_K).with_modifiers(&[Super])),
        Binding::running("bspc node -p east")
            .on(Definition::new(evdev::Key::KEY_L).with_modifiers(&[Super])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_invalid_curly_brace_config() -> Result<(), ParseError> {
    let contents = "
super + }a,b,c{
    {firefox, brave, librewolf}
    ";
    assert_grammar_error_at(contents, (2, 9));
    Ok(())
}

#[test]
// Stricter than previous parser.
fn test_curly_brace_less_commands() -> Result<(), ParseError> {
    let contents = "
super + {a,b,c}
    {firefox, brave}
    ";
    assert_grammar_error_at_span(contents, (2, 1), (3, 21));
    Ok(())
}

#[test]
// Stricter than previous parser.
fn test_curly_brace_less_keysyms() -> Result<(), ParseError> {
    let contents = "
super + {a,b}
    {firefox, brave, librewolf}
    ";
    assert_grammar_error_at_span(contents, (2, 1), (3, 32));
    Ok(())
}

#[test]
// Single variant curly brace shorthands are disallowed.
// We must have at least two variants for shorthands to make sense.
fn test_curly_brace_single_variant() -> Result<(), ParseError> {
    let contents = "
super + {a}
    {firefox}
    ";
    assert_grammar_error_at(contents, (2, 10));
    Ok(())
}

#[test]
fn test_omission() -> Result<(), ParseError> {
    let contents = "
super + {_, shift +} b
    {firefox, brave}";
    let known = [
        Binding::running("firefox").on(Definition::new(evdev::Key::KEY_B).with_modifiers(&[Super])),
        Binding::running("brave")
            .on(Definition::new(evdev::Key::KEY_B).with_modifiers(&[Super, Shift])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_range_syntax() -> Result<(), ParseError> {
    let contents = "
super + {1-9,0}
    bspc desktop -f '{1-9,0}'";

    let known = [
        Binding::running("bspc desktop -f '1'")
            .on(Definition::new(evdev::Key::KEY_1).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '2'")
            .on(Definition::new(evdev::Key::KEY_2).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '3'")
            .on(Definition::new(evdev::Key::KEY_3).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '4'")
            .on(Definition::new(evdev::Key::KEY_4).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '5'")
            .on(Definition::new(evdev::Key::KEY_5).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '6'")
            .on(Definition::new(evdev::Key::KEY_6).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '7'")
            .on(Definition::new(evdev::Key::KEY_7).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '8'")
            .on(Definition::new(evdev::Key::KEY_8).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '9'")
            .on(Definition::new(evdev::Key::KEY_9).with_modifiers(&[Super])),
        Binding::running("bspc desktop -f '0'")
            .on(Definition::new(evdev::Key::KEY_0).with_modifiers(&[Super])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_range_syntax_invalid_range() {
    let contents = "
super + {bc-ad}
    {firefox, brave}
    ";
    assert_grammar_error_at(contents, (2, 10));
}

#[test]
fn test_ranger_syntax_not_full_range() {
    let contents = "
super + {a-}
    {firefox, brave}";
    assert_grammar_error_at(contents, (2, 12));
}

#[test]
fn test_period_escape_binding() -> Result<(), ParseError> {
    let contents = "
super + {\\,, .}
	riverctl focus-output {previous, next}";
    let known = vec![
        Binding::running("riverctl focus-output previous")
            .on(Definition::new(evdev::Key::KEY_COMMA).with_modifiers(&[Super])),
        Binding::running("riverctl focus-output next")
            .on(Definition::new(evdev::Key::KEY_DOT).with_modifiers(&[Super])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_period_binding() -> Result<(), ParseError> {
    let contents = "
super + {comma, period}
	riverctl focus-output {previous, next}";
    let known = vec![
        Binding::running("riverctl focus-output previous")
            .on(Definition::new(evdev::Key::KEY_COMMA).with_modifiers(&[Super])),
        Binding::running("riverctl focus-output next")
            .on(Definition::new(evdev::Key::KEY_DOT).with_modifiers(&[Super])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_bspwm_multiple_shorthands() -> Result<(), ParseError> {
    let contents = "
super + {_,shift + }{h,j,k,l}
	bspc node -{f,s} {west,south,north,east}";
    let known = vec![
        Binding::running("bspc node -f west")
            .on(Definition::new(evdev::Key::KEY_H).with_modifiers(&[Super])),
        Binding::running("bspc node -f south")
            .on(Definition::new(evdev::Key::KEY_J).with_modifiers(&[Super])),
        Binding::running("bspc node -f north")
            .on(Definition::new(evdev::Key::KEY_K).with_modifiers(&[Super])),
        Binding::running("bspc node -f east")
            .on(Definition::new(evdev::Key::KEY_L).with_modifiers(&[Super])),
        Binding::running("bspc node -s west")
            .on(Definition::new(evdev::Key::KEY_H).with_modifiers(&[Super, Shift])),
        Binding::running("bspc node -s south")
            .on(Definition::new(evdev::Key::KEY_J).with_modifiers(&[Super, Shift])),
        Binding::running("bspc node -s north")
            .on(Definition::new(evdev::Key::KEY_K).with_modifiers(&[Super, Shift])),
        Binding::running("bspc node -s east")
            .on(Definition::new(evdev::Key::KEY_L).with_modifiers(&[Super, Shift])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_eq!(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_longer_multiple_shorthands() -> Result<(), ParseError> {
    let contents = "
super + {_, ctrl +} {_, shift +} {1-2}
    riverctl {set, toggle}-{focused, view}-tags {1-2}";
    let known = vec![
        Binding::running("riverctl set-focused-tags 1")
            .on(Definition::new(evdev::Key::KEY_1).with_modifiers(&[Super])),
        Binding::running("riverctl set-focused-tags 2")
            .on(Definition::new(evdev::Key::KEY_2).with_modifiers(&[Super])),
        Binding::running("riverctl toggle-focused-tags 1")
            .on(Definition::new(evdev::Key::KEY_1).with_modifiers(&[Super, Control])),
        Binding::running("riverctl toggle-focused-tags 2")
            .on(Definition::new(evdev::Key::KEY_2).with_modifiers(&[Super, Control])),
        Binding::running("riverctl set-view-tags 1")
            .on(Definition::new(evdev::Key::KEY_1).with_modifiers(&[Super, Shift])),
        Binding::running("riverctl set-view-tags 2")
            .on(Definition::new(evdev::Key::KEY_2).with_modifiers(&[Super, Shift])),
        Binding::running("riverctl toggle-view-tags 1")
            .on(Definition::new(evdev::Key::KEY_1).with_modifiers(&[Super, Shift, Control])),
        Binding::running("riverctl toggle-view-tags 2")
            .on(Definition::new(evdev::Key::KEY_2).with_modifiers(&[Super, Shift, Control])),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_equal_binding_set(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_prefix() -> Result<(), ParseError> {
    let contents = "
super + @1
    1
super + ~2
    2
super + ~@3
    3
super + @~4
    4";
    let known = vec![
        Binding::running("1").on(Definition {
            modifiers: [Super].into_iter().collect(),
            key: Key::new(evdev::Key::KEY_1, KeyAttribute::OnRelease),
        }),
        Binding::running("2").on(Definition {
            modifiers: [Super].into_iter().collect(),
            key: Key::new(evdev::Key::KEY_2, KeyAttribute::Send),
        }),
        Binding::running("3").on(Definition {
            modifiers: [Super].into_iter().collect(),
            key: Key::new(evdev::Key::KEY_3, KeyAttribute::Both),
        }),
        Binding::running("4").on(Definition {
            modifiers: [Super].into_iter().collect(),
            key: Key::new(evdev::Key::KEY_4, KeyAttribute::Both),
        }),
    ];
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_equal_binding_set(parsed.bindings, known);
    Ok(())
}

#[test]
fn test_homerow_special_keys_top() -> Result<(), ParseError> {
    let symbols: [&str; 7] = [
        "Escape",
        "BackSpace",
        "Return",
        "Tab",
        "minus",
        "equal",
        "grave",
    ];

    let keysyms: [evdev::Key; 7] = [
        evdev::Key::KEY_ESC,
        evdev::Key::KEY_BACKSPACE,
        evdev::Key::KEY_ENTER,
        evdev::Key::KEY_TAB,
        evdev::Key::KEY_MINUS,
        evdev::Key::KEY_EQUAL,
        evdev::Key::KEY_GRAVE,
    ];

    let mut contents = String::new();
    for symbol in &symbols {
        contents.push_str(&format!("{}\n    st\n", symbol));
    }
    let known = keysyms
        .iter()
        .map(|k| Binding::running("st").on(Definition::new(*k)))
        .collect();
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_equal_binding_set(parsed.bindings, known);
    Ok(())
}

#[test]
fn internal_commands_do_not_fail_variant_validation() {
    let config = r#"
alt + m
    @enter music

mode music
q
    @escape
endmode
"#;

    assert!(SwhkdParser::from(ParserInput::Raw(&config)).is_ok());
}

#[test]
fn test_all_alphanumeric() -> Result<(), ParseError> {
    let symbols: [&str; 36] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
    ];
    let keysyms: [evdev::Key; 36] = [
        evdev::Key::KEY_A,
        evdev::Key::KEY_B,
        evdev::Key::KEY_C,
        evdev::Key::KEY_D,
        evdev::Key::KEY_E,
        evdev::Key::KEY_F,
        evdev::Key::KEY_G,
        evdev::Key::KEY_H,
        evdev::Key::KEY_I,
        evdev::Key::KEY_J,
        evdev::Key::KEY_K,
        evdev::Key::KEY_L,
        evdev::Key::KEY_M,
        evdev::Key::KEY_N,
        evdev::Key::KEY_O,
        evdev::Key::KEY_P,
        evdev::Key::KEY_Q,
        evdev::Key::KEY_R,
        evdev::Key::KEY_S,
        evdev::Key::KEY_T,
        evdev::Key::KEY_U,
        evdev::Key::KEY_V,
        evdev::Key::KEY_W,
        evdev::Key::KEY_X,
        evdev::Key::KEY_Y,
        evdev::Key::KEY_Z,
        evdev::Key::KEY_0,
        evdev::Key::KEY_1,
        evdev::Key::KEY_2,
        evdev::Key::KEY_3,
        evdev::Key::KEY_4,
        evdev::Key::KEY_5,
        evdev::Key::KEY_6,
        evdev::Key::KEY_7,
        evdev::Key::KEY_8,
        evdev::Key::KEY_9,
    ];

    let mut contents = String::new();
    for symbol in &symbols {
        contents.push_str(&format!("{}\n    st\n", symbol));
    }
    let known = keysyms
        .iter()
        .map(|k| Binding::running("st").on(Definition::new(*k)))
        .collect();
    let parsed = SwhkdParser::from(ParserInput::Raw(&contents))?;
    assert_equal_binding_set(parsed.bindings, known);
    Ok(())
}
