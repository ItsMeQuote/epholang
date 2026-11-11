<p align="center">
  <img src="images/epholang_logo.png" alt="Epholang logo">
</p>

<div align="center">
  <h1>Epholang</h1>
</div>

<p align="center">
  An experimental programming language that values <strong>explicitness, readability, and strict static checking</strong>.
</p>

<p align="center">
  <a href="https://github.com/ItsMeQuote/epholang/releases"><img src="https://img.shields.io/badge/version-0.2.0%20(Alpha)-blue" alt="Version"></a>
  <img src="https://img.shields.io/badge/implemented%20in-Rust-orange" alt="Rust">
  <a href="https://github.com/ItsMeQuote/epholang/stargazers"><img src="https://img.shields.io/github/stars/ItsMeQuote/epholang" alt="Stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ItsMeQuote/epholang" alt="License"></a>
  <a href="https://github.com/ItsMeQuote/epholang/actions/workflows/ci.yml"><img src="https://github.com/ItsMeQuote/epholang/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
</p>

<p align="center"><strong>English</strong> &bull; <a href="README.ru.md">Русский</a></p>

---

## Why Epholang?

Epholang is designed around a few deliberate choices:

- **One way to do a thing.** Avoid needless overlap between features.
- **Explicit over short.** Changes to data and types should be visible in code.
- **Strict, not forgiving.** Type mismatches, unused names, and missing returns are errors, not warnings.
- **Tradition is not an argument.** Lists are indexed from **1**. Negative numbers are written as `(-1)`.
- **No classical OOP.** There are no classes, inheritance, or methods.

Epholang is a **tree-walking interpreter** with a static checking pass before execution. It is an experimental language: incompatible changes are possible.

## A taste of Epholang

```epholang
import "math" as math

function Square(value int) int {
    return int value * value
}

numbers ls<int> := list<int>{1, 2, 3}
loop number in numbers {
    Output("#{number} squared = #{Square(number)}\n")
}

Output("Sum = #{math:Sum(numbers)}\n")
```

```text
1 squared = 1
2 squared = 4
3 squared = 9
Sum = 6
```

The language uses explicit types (`int`, `flt`, `str`, `bool`), typed functions, string interpolation, one-based lists, and built-in modules.

Function names follow **PascalCase by convention**, while variables and module aliases typically use **camelCase**. Naming style is _not_ enforced by the interpreter.

Find seven ready-to-run programs in [`examples/`](examples/) and the extended language guide in the [GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki).

## Getting started

### Download or build

Download available executables and the VS Code extension from [Releases](https://github.com/ItsMeQuote/epholang/releases).

To build from source, install a stable [Rust toolchain](https://www.rust-lang.org/tools/install):

```sh
git clone https://github.com/ItsMeQuote/epholang.git
cd epholang
cargo build --release
```

The binary is located at `target/release/epholang` (Windows: `target\release\epholang.exe`).

To build a specific version, check out its corresponding Git tag first.

### CLI

```sh
epholang run program.epho      # statically check, then run
epholang check program.epho    # static checks only
epholang tokens program.epho   # inspect lexer tokens
epholang parse program.epho    # inspect the syntax tree
epholang format program.epho   # format source with EphoFMT
epholang format --check program.epho # check formatting without changes
```

To try an example without adding the binary to `PATH`:

```sh
cargo run -- run examples/03_loops.epho
```

### VS Code

Install the Epholang VS Code extension (`.vsix`) from [Releases](https://github.com/ItsMeQuote/epholang/releases) via **Extensions → … → Install from VSIX...**.

Alternatively, run the following command with the downloaded file in the current directory, replacing the filename if necessary:

```sh
code --install-extension epholang-language-0.2.0.vsix
```

The extension provides syntax highlighting, snippets, and EphoFMT document formatting. Formatting on save is enabled by default for Epholang files; **Shift + Alt + F** runs it manually. The extension invokes the installed Epholang CLI. If it is not available on `PATH`, set `epholang.executablePath` to its executable path. The extension is not a compiler or language server.

## Language at a glance

| Feature     | Example / rule                                                           |
| ----------- | ------------------------------------------------------------------------ |
| Numbers     | `int` (`i64`) and `flt` (`f64`); no implicit mixing                      |
| Other types | `str`, `bool`, `none`, and type values (`type`)                          |
| Lists       | `ls<int>`, `ls<flt>`, `ls<str>`, `ls<bool>`, `ls<none>` — one level deep |
| Variables   | `age int := 20` and `age = 21`                                           |
| Functions   | `function Square(x int) int { return int x * x }`                        |
| Conditions  | `if ... { } otherwise if ... { } otherwise { }`                          |
| Loops       | `loop condition { }` or `loop item in items { }`                         |
| Strings     | `Output("Hello, #{name}!\n")`                                            |
| Comments    | `/* block comments /* can nest */ */`                                    |

**Functions** require explicit parameter and return types, an explicitly typed `return`, and a return on every path. Default arguments are not supported. Functions are declared at top level before use.

**Lists** start at index **1** and require an `int` index. `Insert` and `Remove` return new lists.

**Input and output** do not add a newline automatically. Use `\n` explicitly.

**Arithmetic** distinguishes `int` and `flt`. An `int / int` expression returns `flt` only when its mathematical quotient is exactly representable in binary64 (`5 / 2` works; `1 / 3` raises a runtime error). Integer overflow is an error, and mixed numeric operations require explicit conversion.

## Built-in functions

| Function                     | Purpose                                  |
| ---------------------------- | ---------------------------------------- |
| `Output(value)`              | Print without an automatic newline       |
| `Input(prompt)`              | Read a line of text as `str`             |
| `Length(value)`              | Count string characters or list elements |
| `Includes(container, value)` | Test for a substring or list element     |
| `Insert(list, index, value)` | Return a list with an inserted element   |
| `Remove(list, index)`        | Return a list without an element         |
| `TypeOf(value)`              | Get a value's type                       |

## Standard modules

Import modules at the beginning of the program and call their functions with `alias:Function(...)`:

```epholang
import "strings" as strings
import "math" as math
import "types" as types

Output(strings:ToUpperCase("hello"))
Output("\n")
Output("Random: #{math:RandomInterval(1, 6)}\n")
Output("Convertible: #{types:IsConvertible(\"42\", int)}\n")
```

| Module    | Functions                                                                                                                  |
| --------- | -------------------------------------------------------------------------------------------------------------------------- |
| `strings` | `Trim`, `Replace`, `Split`, `StartsWith`, `EndsWith`, `ToLowerCase`, `ToUpperCase`, `Capitalize`, `Repeat`                 |
| `math`    | `AbsoluteValue`, `SquareRoot`, `Floor`, `Ceil`, `Round`, `Minimum`, `Maximum`, `Clamp`, `Sum`, `Average`, `RandomInterval` |
| `types`   | `Convert`, `IsConvertible`                                                                                                 |

`math:RandomInterval(minimum, maximum)` returns an `int` with **both bounds included**. It is not intended for cryptographic use.

`types:Convert(value, targetType)` and `types:IsConvertible(value, targetType)` require a literal target type (`int`, `flt`, `str`, or `bool`).

See the [GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki) for function arguments, edge cases, and more examples.

## Clear errors, early

Epholang distinguishes three error stages:

| Error           | When                | Reporting           |
| --------------- | ------------------- | ------------------- |
| `Syntax error`  | Reading source code | First error         |
| `Static error`  | Before execution    | All detected errors |
| `Runtime error` | During execution    | First error         |

Diagnostics point to the affected file, line, column, and source span.

Static checks include type mismatches, unused variables, and missing returns.

## From 0.1.0 to 0.2.0

**Epholang 0.2.0 introduces breaking changes** to its syntax, type system, and built-in functions.

See [CHANGELOG.md](CHANGELOG.md) for the complete list of changes. Detailed language documentation is available in the [GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki).

## Current limits

The following features are not implemented:

- Union types
- Nested lists
- User-defined modules
- Error handling as values
- Custom types
- Native compilation

## About Epholang

The implementation of Epholang was written **with AI assistance**.

The language design — including its syntax, semantics, rules, and design decisions — comes from [ItsMeQuote](https://github.com/ItsMeQuote).

Updates and discussion (in Russian): [t.me/epholang](https://t.me/epholang).

Licensed under [MIT](LICENSE).
