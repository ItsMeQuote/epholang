<p align="center">
  <img src="images/epholang_logo.png" alt="Epholang logo">
</p>

<div align="center">
  <h1>Epholang</h1>
</div>

<p align="center">An experimental programming language focused on explicit code and strict static checking.</p>

<p align="center">
  <a href="https://github.com/ItsMeQuote/epholang/releases"><img src="https://img.shields.io/badge/status-0.2.0%20alpha%20in%20development-yellow" alt="Development status"></a>
  <img src="https://img.shields.io/badge/implementation-Rust-orange" alt="Implementation language">
  <a href="https://github.com/ItsMeQuote/epholang/stargazers"><img src="https://img.shields.io/github/stars/ItsMeQuote/epholang" alt="GitHub stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ItsMeQuote/epholang" alt="License"></a>
  <a href="https://github.com/ItsMeQuote/epholang/actions/workflows/ci.yml"><img src="https://github.com/ItsMeQuote/epholang/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
</p>

---

[**English**](README.md) | [Русский](README.ru.md)

> **Status:** Epholang **0.2.0 is under development**. This README documents the tested 0.2.0 Stage 3 implementation, **not a published 0.2.0 release**. The [Releases page](https://github.com/ItsMeQuote/epholang/releases) is the source of truth for downloadable versions. The language is experimental; breaking changes are expected.
>
> The implementation is a **tree-walking interpreter with a static checking pass**, not a native compiler.

## Philosophy

Epholang is designed around a few principles:

- **One way to do a thing.** Avoid overlapping features where possible.
- **Explicit over short.** Readable, deliberate code matters more than brevity.
- **Strict, not forgiving.** Unused variables, type mismatches, and missing returns are errors rather than warnings.
- **Visible changes.** State changes should be obvious in source code.
- **Tradition is not an argument.** List indexing starts at **1**; negative numbers use parentheses such as `(-1)`.
- **No classical OOP.** No classes, inheritance, or methods.

Function names follow **PascalCase** by convention; variable names, parameter names, and module aliases typically use **camelCase**. This is **not enforced by the interpreter**. Keywords and built-in types are lowercase.

## Quick start

### Example: functions and interpolation

```epholang
function Greet(name str) str {
    if name == "" {
        return str "Hello!"
    } otherwise {
        return str "Hello, #{name}!"
    }
}

message str := Greet("World")
Output("#{message}\n")
```

```text
Hello, World!
```

### Example: lists and math

```epholang
import "math" as math

numbers ls<int> := list<int>{10, 20, 30}
Output("Count: #{Length(numbers)}\n")
Output("Sum: #{math:Sum(numbers)}\n")
Output("Average: #{math:Average(numbers)}\n")
```

```text
Count: 3
Sum: 60
Average: 20
```

More examples are available in [`examples/`](examples/), including [`stage3.epho`](examples/stage3.epho).

## Install and run

### Download

Check the [GitHub Releases](https://github.com/ItsMeQuote/epholang/releases) page for **published** binaries. A Windows build and VS Code extension for 0.1.0 were published previously; do not assume the 0.2.0 files are available until a corresponding release is published.

### Build from source

Install a stable [Rust toolchain](https://www.rust-lang.org/tools/install), then, using a repository checkout containing the **0.2.0 source**:

```sh
git clone https://github.com/ItsMeQuote/epholang.git
cd epholang
cargo build --release
```

> The default branch may still contain 0.1.0 until the 0.2.0 changes are pushed. Check the branch or tag before building.

On Windows the resulting executable is `target\release\epholang.exe`; on Unix-like systems it is generally `target/release/epholang` for the host platform.

### CLI

```sh
epholang run program.epho      # static check, then execute
epholang check program.epho    # static check only
epholang tokens program.epho   # display lexer output
epholang parse program.epho    # display the syntax tree
```

If the executable isn't on `PATH`, run it from `target/release` or with Cargo, e.g. `cargo run -- run examples/stage3.epho`.

### VS Code

The 0.2.0 extension provides syntax highlighting for `.epho`, updated built-in names, module calls, and snippets. Once the new VSIX is packaged or published, install it from **Extensions → … → Install from VSIX**, or use:

```sh
code --install-extension epholang-language-0.2.0.vsix
```

The command requires that VSIX file to be present locally. An older 0.1.0 extension does not fully describe the new syntax.

## Language overview

### Types and variables

| Type    | Meaning                                                            |
| ------- | ------------------------------------------------------------------ |
| `int`   | Signed 64-bit integer (`i64`)                                      |
| `flt`   | IEEE 754 binary64 floating-point (`f64`)                           |
| `str`   | String                                                             |
| `bool`  | Boolean                                                            |
| `none`  | Absence of a value (`null`)                                        |
| `type`  | Type value (e.g. the result of `TypeOf`)                           |
| `ls<T>` | One-level list where `T` is `int`, `flt`, `str`, `bool`, or `none` |

```epholang
age int := 17
age = 18
price flt := 19.99
name str := "Epholang"
active bool := true
Output("#{name}: #{age}, #{price}, #{active}\n")
```

Variables are explicitly declared with `:=`; their declared types cannot change. Variables are block-scoped, and shadowing a name from an outer scope is a static error. Implicit conversion between `int` and `flt` is not supported.

### Functions

```epholang
function Square(value int) int {
    return int value * value
}

function Log(message str) none {
    Output("[log] #{message}\n")
    return none null
}
```

- Use `function Name(parameters) returnType { ... }`; there is **no `:=`** before the body.
- Every parameter and return type is explicit, and every `return` specifies a type.
- Functions must return a value along every path, including functions returning `none`.
- Default parameter values are **not supported** in 0.2.0.
- Functions must be declared at top level before use; assigning to global variables inside functions is forbidden.
- PascalCase is a naming convention, not a static-checking requirement.

### Conditions and loops

```epholang
value int := 8
if value > 10 {
    Output("large\n")
} otherwise if value > 5 {
    Output("medium\n")
} otherwise {
    Output("small\n")
}

i int := 1
cycle i < 4 {
    Output("#{i}\n")
    i = i + 1
}

items ls<int> := list<int>{10, 20}
cycle item in items {
    Output("#{item}\n")
}
```

Conditions must have type `bool`. The operators `<=` and `>=` are not available; use equivalent combinations of `<`, `>`, `!`, and equality checks.

### Lists

```epholang
items ls<str> := list<str>{"a", "b"}
Output(items[1])                     /* first element */
items[1] = "z"
items = Insert(items, 2, "c")       /* returns a new list */
items = Remove(items, 1)            /* returns a new list */
Output("#{Length(items)}\n")
```

- List indices are **1-based** and require `int`; zero and negative indices are invalid.
- List assignment produces an independent copy.
- Lists can be compared for equality by type and ordered contents.
- Nested lists and union element types are not implemented.

### Strings, input and output

```epholang
name str := Input("Your name: ")
Output("Hello, #{name}!\n")
Output("Literal interpolation: \#{name}\n")
Output("Characters: #{Length(name)}\n")
```

Neither `Input` nor `Output` inserts a newline automatically. `Input(prompt)` returns `str`, removing the line ending produced by Enter but preserving other characters, including spaces. `Length(str)` counts Unicode scalar values (not grapheme clusters). Use `\n` for an explicit newline; string concatenation with `+` is not supported.

### Numbers and arithmetic

```epholang
negative int := (-5)
quotient flt := 5 / 2            /* 2.5 */
Output("#{2 ^ 3 ^ 2}\n")        /* 512: right-associative power */
Output("#{7 % 3}\n")
```

- Literal `5` is `int`; `5.0` is `flt`.
- Arithmetic and comparisons between `int` and `flt` require explicit conversion. The exception is `int / int`, whose result is `flt`.
- **Important:** `int / int` succeeds only when the exact quotient can be represented by `f64`. `5 / 2` works; **`1 / 3` is a runtime error**. This provisional restriction is intentional in the current implementation.
- Integer overflow and division by zero produce runtime errors; negative integer exponents are not supported.
- Floating-point results must be finite. Floating-point arithmetic has ordinary binary64 precision limits.

### Comments

Only block comments are supported; they can be nested:

```epholang
/* outer comment
    /* nested comment */
    still a comment
*/
```

## Built-in functions

| Function                     | Result / behavior                                                  |
| ---------------------------- | ------------------------------------------------------------------ |
| `Output(value)`              | Prints a value without adding a newline; returns `none`            |
| `Input(prompt)`              | Prints a `str` prompt and reads a line as `str`                    |
| `Length(value)`              | `int`: number of characters in a string or items in a list         |
| `Includes(container, value)` | `bool`: substring search in `str`, or element membership in a list |
| `Insert(list, index, value)` | New list containing an inserted element                            |
| `Remove(list, index)`        | New list with an element removed                                   |
| `TypeOf(value)`              | A value of type `type` representing the argument's type            |

`Includes("abc", "")` is rejected because an empty substring pattern is not supported; `Includes(list<str>{""}, "")` is valid.

## Standard modules

Import a built-in module **at the beginning of the file** with an alias:

```epholang
import "strings" as strings
import "math" as math
import "types" as types
```

A module may be imported only once per file. Only these three built-in modules are available; user-defined modules are not yet supported. Calls use the `alias:Function(...)` syntax, with no spaces around `:`.

### `strings`

| Function                    | Description                                           |
| --------------------------- | ----------------------------------------------------- |
| `Trim(text, pattern)`       | Repeatedly removes a nonempty pattern from both ends  |
| `Replace(text, old, new)`   | Replaces every occurrence of nonempty `old`           |
| `Split(text, separator)`    | Splits text into `ls<str>` using a nonempty separator |
| `StartsWith(text, pattern)` | Checks a nonempty prefix                              |
| `EndsWith(text, pattern)`   | Checks a nonempty suffix                              |
| `ToLowerCase(text)`         | Converts text to lowercase                            |
| `ToUpperCase(text)`         | Converts text to uppercase                            |
| `Capitalize(text)`          | Uppercases the first alphabetic character             |
| `Repeat(text, count)`       | Repeats text `count` times; count must be nonnegative |

`Repeat` limits the resulting string to **64 MiB**.

### `math`

| Function                   | Result / behavior                                     |
| -------------------------- | ----------------------------------------------------- |
| `AbsoluteValue(number)`    | `int` or `flt`, same type; checks integer overflow    |
| `SquareRoot(number)`       | `flt → flt`; rejects negative inputs                  |
| `Floor(number)`            | `flt → flt`                                           |
| `Ceil(number)`             | `flt → flt`                                           |
| `Round(number)`            | `flt → flt`; ties are rounded away from zero          |
| `Minimum(a, b)`            | Smaller of two same-type numeric values               |
| `Maximum(a, b)`            | Larger of two same-type numeric values                |
| `Clamp(value, min, max)`   | Restricts a number to a valid same-type interval      |
| `Sum(numbers)`             | Sum of `ls<int>` or `ls<flt>`, retaining element type |
| `Average(numbers)`         | `flt` average of a nonempty numeric list              |
| `RandomInterval(min, max)` | Random `int` with **both bounds included**            |

For `Average(ls<int>)`, the quotient currently must be exactly representable as `flt`; otherwise a runtime error is raised. This rule is subject to future design review. `RandomInterval` is **not intended for security-sensitive randomness**.

### `types`

```epholang
import "types" as types

raw str := "42"
if types:IsConvertible(raw, int) {
    parsed int := types:Convert(raw, int)
    Output("#{parsed}\n")
}
```

| Function                           | Behavior                                                                                                                     |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `Convert(value, targetType)`       | Returns the value converted to the statically specified target type; raises a runtime error if the value cannot be converted |
| `IsConvertible(value, targetType)` | Checks whether the particular value can be converted; returns `bool`                                                         |

The **second argument must be a literal type**: `int`, `flt`, `str`, or `bool`. Conversion is explicit; only defined pairs are supported. `flt → int` requires a whole, in-range value. `int → flt` must preserve integer precision. String parsing into `flt` uses binary64 representation and can be approximate. List conversions are not supported.

## Diagnostics and testing

Syntax errors report the first problem during parsing, static checking reports all detected static errors before execution, and a runtime error stops execution at the first runtime failure. Diagnostics include a file, line, column, and source excerpt.

The Stage 3 implementation was checked on Windows with **25 unit tests + 17 CLI integration tests (42 passed)**, and `cargo fmt --check` / Clippy were reported clean. This is test coverage, **not a guarantee of correctness on every platform**.

From the project root:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -- run examples/stage3.epho
```

See [`TESTING.md`](TESTING.md) for additional test details if that file is included in the checkout.

## Migrating from 0.1.0

**0.2.0 contains breaking changes.** Existing programs may require edits:

| 0.1.0                                            | 0.2.0                                            |
| ------------------------------------------------ | ------------------------------------------------ |
| `num`                                            | `int` or `flt`, chosen explicitly                |
| `x func := function(a num) num { ... }`          | `function X(a int) int { ... }`                  |
| Default parameter values                         | Removed                                          |
| `output`, `length`, `insert`, `remove`, `typeOf` | `Output`, `Length`, `Insert`, `Remove`, `TypeOf` |
| Numeric indices as `num`                         | Indices require `int`                            |
| No standard modules                              | `strings`, `math`, and `types` imports           |

Also review mixed arithmetic, exact `int / int` division, and the new names of other built-in functions. See the release notes for any changes made after this Stage 3 snapshot.

## Not implemented yet

- Union types (structural groundwork exists, but union syntax and checking are not enabled)
- Nested lists
- Error handling as values
- Custom types
- Importing user-defined modules
- A native-code compiler

## About the project

Epholang's implementation was written **with the assistance of AI**. The language itself—its syntax, semantics, rules, and design decisions—was created by [ItsMeQuote](https://github.com/ItsMeQuote).

## Community

Development updates and discussion (in Russian): [t.me/epholang](https://t.me/epholang).

## License

[MIT](LICENSE).
