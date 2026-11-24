<p align="center">
  <img src="images/epholang_logo_icon.png" alt="Epholang logo">
  <img style="width: 3%; height: auto;">
  <img src="images/epholang_hexer.png" alt="Hexer">
</p>

<div align="center">
  <h1>Epholang</h1>
</div>

<p align="center">
  An experimental programming language that values <strong>explicitness, readability, and strict static checking</strong>.
</p>

<p align="center">
  <a href="https://github.com/ItsMeQuote/epholang/releases"><img src="https://img.shields.io/badge/version-0.3.0%20(Beta)-blue" alt="Version"></a>
  <img src="https://img.shields.io/badge/implemented%20in-Rust-orange" alt="Rust">
  <a href="https://github.com/ItsMeQuote/epholang/stargazers"><img src="https://img.shields.io/github/stars/ItsMeQuote/epholang" alt="Stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ItsMeQuote/epholang" alt="License"></a>
  <a href="https://github.com/ItsMeQuote/epholang/actions/workflows/ci.yml"><img src="https://github.com/ItsMeQuote/epholang/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
</p>

<p align="center"><strong>English</strong> &bull; <a href="README.ru.md">Russian</a></p>

---

[Beta handbook](https://github.com/ItsMeQuote/epholang/wiki/Handbook) · [Roadmap](ROADMAP.md) · [Installation](https://github.com/ItsMeQuote/epholang/wiki/Installation) · [Tutorial](https://github.com/ItsMeQuote/epholang/wiki/Hello-World) · [Language reference](https://github.com/ItsMeQuote/epholang/wiki/Syntax-Overview) · [Standard library](https://github.com/ItsMeQuote/epholang/wiki/Standard-Library) · [CLI and VS Code](https://github.com/ItsMeQuote/epholang/wiki/CLI-Reference) · [Migration from 0.2 to 0.3](MIGRATION.md)

## Why Epholang?

<img src="images/epholang_hexer_question.png" align="right" alt="Hexer with a question mark">

Epholang is designed around a few deliberate choices:

- **One way to do a thing.** Avoid needless overlap between features.
- **Explicit over short.** Changes to data and types should be visible in code.
- **Strict, not forgiving.** Type mismatches, unused names, and missing returns are errors, not warnings.
- **Tradition is not an argument.** Lists are indexed from **1**. Negative numbers are written as `(-1)`.
- **No classical OOP.** There are no classes, inheritance, or methods.

Epholang is a **tree-walking interpreter** with a static checking pass before execution. It is an experimental language: incompatible changes are possible.

## A taste of Epholang

<img src="images/epholang_hexer_laptop.png" align="right" alt="Hexer is holding a laptop">

```epholang
import "math" as math

function Square(int value) -> int:
	return int value * value

ls<int> numbers := list<int>{1, 2, 3}
loop number in numbers:
	Output("#{number} squared = #{Square(number)}\n")

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

<img src="images/epholang_hexer_arrow.png" align="right" alt="Hexer with an arrow">

### Download or build

Build the interpreter from the same source checkout to run the examples in this guide. Published builds and matching extensions are available from [Releases](https://github.com/ItsMeQuote/epholang/releases).

To build from source, install a stable [Rust toolchain](https://www.rust-lang.org/tools/install):

```sh
git clone https://github.com/ItsMeQuote/epholang.git
cd epholang
cargo build --release --locked
```

The binary is located at `target/release/epholang` (Windows: `target\release\epholang.exe`).

To build a specific version, check out its corresponding Git tag first.

### CLI

```sh
epholang run program.epho            # statically check, then run
epholang check program.epho          # static checks only
epholang tokens program.epho         # inspect lexer tokens
epholang parse program.epho          # inspect the syntax tree
epholang format program.epho         # format source with EphoFMT
epholang format --check program.epho # check formatting without changes
```

To try an example without adding the binary to `PATH`:

```sh
cargo run -- run examples/03_loops.epho
```

### VS Code

Use the extension and CLI from the same version of Epholang. To try the extension from this checkout, open a VS Code Extension Development Host with `--extensionDevelopmentPath` set to the full path of `editors/vscode`. If the matching CLI is outside your `PATH`, set `epholang.executablePath` to its full path.

Epholang files use tabs and format on save by default. The VS Code **Format Document** command also invokes EphoFMT. The extension highlights `-|` and the discard marker. EphoFMT formats `_=F()` as `_ = F()`. Use `check` to verify that `F()` produces a value.

## Language at a glance

| Feature     | Example / rule                                                           |
| ----------- | ------------------------------------------------------------------------ |
| Numbers     | `int` (`i64`) and `flt` (`f64`); no implicit mixing                      |
| Other types | `str`, `bool`, `none`, and type values (`type`)                          |
| Lists       | `ls<int>`, `ls<flt>`, `ls<str>`, `ls<bool>`, `ls<none>` — one level deep |
| Variables   | `int age := 20` and `age = 21`                                           |
| Functions   | `function Square(int x) -> int:`                                         |
| Conditions  | `if …: / else if …: / else:`                                             |
| Loops       | `loop condition:` or `loop item in items:`                               |
| Strings     | `Output("Hello, #{name}!\n")`                                            |
| Comments    | `/* block comments /* can nest */ */`                                    |

**Blocks** start on the line after a header ending in `:`. Use one tab per nesting level. Blank lines and comment-only lines do not set a level. Code after the closing delimiter of a multiline comment on the same line inherits the indentation from the comment’s opening line. Use tabs on continuation lines inside calls, indexes and lists. EphoFMT preserves multiline lists and adds a trailing comma after their last element.

**Strings** support nested interpolation, such as `"Result: #{strings:Repeat("Hi #{name}", 2)}"`. Keep each interpolation on one physical line. Escapes such as `\n` and `\"` are still available in string text.

**Functions** are declared at the top level before use. Each parameter has an explicit type. Default arguments and recursion are forbidden. In a function declared with `-> T`, every path must return a value with a typed `return T expression`. The marker `-|` is valid only in a function header. Such a function finishes at the end of its body and forbids every form of `return`.

```epholang
function Display(str message) -|:
	Output(message)

function Find() -> none:
	return none null

Display("Ready\n")
none result := Find()
Output(result)
_ = Find()
```

Here `Find()` returns the value `null` of type `none`. You can store, print, or explicitly discard it. `Display()` and `Output()` produce no value. Call them as statements. Assigning their result, passing it as an argument, or writing `_ = Display("Ready")` is a static error.

**Using results.** Every expression that produces a value must use it in context. For example, standalone `42`, `result`, and `Length("abc") + 1` are errors. Use `_ = expression` to evaluate an expression once for its effects and discard its value. Errors propagate normally. A single `_` is valid only on the left of this statement. It creates no binding. Both `_ := value` and `result = _` are errors. Epholang has no operation to delete declarations through `_`. Names such as `_item` follow the ordinary identifier rules.

**Union types** use `|`. For example, `int | str` allows an integer or a string and `ls<int | str>` allows both types as list elements.

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

## Standard modules

Import modules at the beginning of the program and call their functions with `alias:Function(...)`:

```epholang
import "strings" as strings
import "math" as math
import "types" as types

Output(strings:ToUpperCase("hello"))
Output("\n")
Output("Random: #{math:RandomInterval(1, 6)}\n")
Output("Convertible: #{types:IsConvertible("42", int)}\n")
```

| Module    | Functions                                                                                                                  |
| --------- | -------------------------------------------------------------------------------------------------------------------------- |
| `strings` | `Trim`, `Replace`, `Split`, `StartsWith`, `EndsWith`, `ToLowerCase`, `ToUpperCase`, `Capitalize`, `Repeat`, `AlphabetRange`, `IsAsciiUppercase`, `IsAsciiLowercase` |
| `math`    | `AbsoluteValue`, `SquareRoot`, `Floor`, `Ceil`, `Round`, `Minimum`, `Maximum`, `Clamp`, `Sum`, `Average`, `RandomInterval` |
| `types`   | `Convert`, `IsConvertible`, `DeclaredTypeOf`, `CurrentTypeOf`                                                              |

`math:RandomInterval(minimum, maximum)` returns an `int` with **both bounds included**. It is not intended for cryptographic use.

`types:Convert(value, targetType)` and `types:IsConvertible(value, targetType)` require a literal target type (`int`, `flt`, `str`, `bool`, or `ls<str>`).

See the [GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki) for function arguments, edge cases, and more examples.

## Clear errors, early

<img src="images/epholang_hexer_error.png" align="right" alt="Hexer with an error">

Epholang distinguishes three error stages:

| Error           | When                | Reporting           |
| --------------- | ------------------- | ------------------- |
| `Syntax error`  | Reading source code | First error         |
| `Static error`  | Before execution    | All detected errors |
| `Runtime error` | During execution    | First error         |

Diagnostics point to the affected file, line, column, and source span.

Every user declaration needs a syntactic reference after it is declared. This includes parameters, loop variables, import aliases and names starting with `_`. A valid assignment counts as a reference, as does a reference in a branch that will never execute.

## Upgrading to 0.3.0 Beta

For 0.2 source, migrate block indentation and function signatures first. Then review every standalone expression and every caller that expected a value from `Output`. The [migration guide](MIGRATION.md) pairs old and new syntax. The [diagnostics guide](https://github.com/ItsMeQuote/epholang/wiki/Diagnostics) explains the messages you may encounter when checking the migrated file.

## From 0.1.0 to 0.2.0

**Epholang 0.2.0 introduces breaking changes** to its syntax, type system, and built-in functions.

See [CHANGELOG.md](CHANGELOG.md) for the complete list of changes. Detailed language documentation is available in the [GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki).

## Current limits

The following features are not implemented:

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
