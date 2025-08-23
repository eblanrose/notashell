# ansSH

![Build Status](https://img.shields.io/badge/build-passing-brightgreen)
![Rust Version](https://img.shields.io/badge/rust-1.80+-blue)
![License](https://img.shields.io/badge/license-MIT-lightgrey)

`ansSH` is a custom shell written in Rust, inspired by `sh` and `bash`. It supports aliases, history, autocompletion, glob patterns, syntax highlighting, and recursive `$()` commands.

---

## Features

- Support for command pipelines (`|`)
- Autocompletion for files, directories, and binaries
- Syntax highlighting based on configurable rules
- Aliases and environment variables
- Expansion of `~` to the home directory
- Glob pattern (`*`) expansion
- Recursive `$()` command substitution
- Persistent command history

---

## Installation

Build using Cargo:

```bash
git clone https://github.com/akaruinekooff/ansSH.git
cd ansSH
cargo build --release
````

The binary will be located at `target/release/ansSH`.

---

## Usage

Start the shell:

```bash
./target/release/ansSH
```

Examples:

```bash
anssh> echo $(nproc)
12
anssh> ga .
anssh> gc "added new features"
anssh> gp
```

---

## Configuration

Configuration files are located in `~/.config/anssh` and `~/.anssh_rc`.

### `~/.anssh_rc` (aliases and environment variables)

```bash
# Environment variables
EDITOR="ansIDE"
HI="Hello, world!"

# Aliases
alias ga="git add"
alias gc="git commit -m"
alias gp="git push"
```

### `~/.config/anssh/complete` (autocompletion rules)

```text
# Complete directories
$directory

# Complete files
$file

# Ignore binaries
!ls
!cat
```

### `~/.config/anssh/highlight_rules` (syntax highlighting)

```text
*.rs:green
*.sh:blue
*.toml:yellow
```

### `~/.config/anssh/prompt` (prompt customization)

```text
$green{user}$clear:$blue{cwd}$clear >
```

You can use `$blue`, `$green`, `$red`, `$yellow`, `$cyan`, `$white`, and `$clear` to add colors to your prompt.

(I haven’t tested these configurations, so if any issues appear, please open an issue)

---

## License

MIT License © AkaruiNeko