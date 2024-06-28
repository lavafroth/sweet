# `sweet`
Simple Wayland Event Encoding Text

V2 config file parser for [swhkd](https://github.com/waycrate/swhkd.git).
Developed through Google Summer of Code 2024.

### What it does

This sweet little parser is to be gradually integrated into the swhkd repo itself.
For now, the grammar for the config parser is being implemented in this isolated
repository.

As of now, the parser prints all the bindings that a given inputs file can expand to.

### Progress

Here's what has been worked on so far.

- [x] Bindings
  - [x] Modifiers
  - [x] Regular keys
  - [x] Shorthands
  - [x] Ranges
  - [x] Omissions
  - [x] _Send_ and _on release_ attributes
- [x] Unbinds
- [x] Modes
  - [x] Oneoff
  - [x] Swallow
  - [ ] `@mode` in commands (Grammar implemented)
- [x] Comments
- [x] Imports
  - [x] Merge definitions from all imports
- [ ] Tests (WIP)
- [ ] Integration into downstream [WIP](https://github.com/lavafroth/swhkd/tree/sweet-integration)

## Extra features
- [x] Warn user if input config is not a regular file
- [x] Set a maximum file size cap for configs
- [ ] Lock files and map them into memory
- [ ] Map keys and modifiers to internal representation (evdev enum variants) in a single pass


Want to learn how the code works? Check out [my blog](https://lavafroth.is-a.dev/tags/google-summer-of-code/) where I cover each topic as I implement them.

### See it in action

All the examples that are currently tested against are piled up in the
`hotkeys.swhkd` file. These are to be ported to actual cargo tests.

To see a structured representation of the sample config file after parsing, run the following:

```
cargo r -- hotkeys.swhkd
```

To run all available tests (some of which are still being ported), run

```
cargo test
```

### Contributing

This repo is for cathedral style development. I'm assigned with creating the parser and there's potential financial transactions involved. Contributions are NOT welcome.
