# Epholang 0.3 language reference

This reference describes Epholang 0.3.0 Beta. Start with the [tutorial](Tutorial.md) for a complete program or the [migration guide](../MIGRATION.md) for changes from 0.2.

## Source, blocks and names

Save UTF-8 source in a file ending in `.epho`. A block starts after a header ending in a colon and a newline. Indent each nested level with one literal tab. A block must contain a statement. Dedent to the enclosing level to end the block. Blank lines and comment-only lines do not change the block depth. Semicolons can separate simple statements on the same line. EphoFMT expands them onto separate lines.

Block comments use `/* ... */` and may nest. Identifiers are case-sensitive and use ASCII letters, digits and underscores. They start with a letter or underscore. A lone underscore is reserved for discard. PascalCase for functions and camelCase for variables and module aliases are conventions.

Program execution starts at the top level. Imports go first. Declare functions and variables before their first reference. A reference to an identifier with a leading underscore, such as `_count`, is still required for the declaration to pass static checking.

## Values and declarations

For examples of nullable values, parameter types and block scope, see [types and scope](Types-and-Scope.md).

| Type | Example | Purpose |
| --- | --- | --- |
| `int` | `42` | Signed 64-bit integer |
| `flt` | `2.5` | Finite 64-bit floating-point number |
| `str` | `"Epholang"` | Unicode text |
| `bool` | `true` | Boolean |
| `none` | `null` | The null value |
| `ls<T>` | `list<int>{1, 2}` | One-level list |
| `type` | `int` in an expression | A type value used by introspection and conversion |

Put the declared type before the name. Use `:=` for initialization and `=` for later assignment. A union such as `int | str` accepts any of its members. The declared type stays the same when the current value changes. `type` values can be passed to the built-in type operations and printed directly.

```epholang
import "types" as types

int | str item := 42
Output("#{types:DeclaredTypeOf(item)} / #{types:CurrentTypeOf(item)}\n")
item = "answer"
Output("#{types:DeclaredTypeOf(item)} / #{types:CurrentTypeOf(item)}\n")
```

Expected output

```text
int | str / int
int | str / str
```

## Expressions and operators

| Precedence, high to low | Operations |
| --- | --- |
| Primary and postfix | Literals, parentheses, calls and indexing |
| Unary | `!value` for Boolean negation, `(-value)` for numeric negation |
| Power | `^` |
| Multiplicative | `*`, `/`, `%` |
| Additive | `+`, `-` |
| Comparison | `<`, `>`, `==`, `!=` |
| Logical AND | `&&` |
| Logical OR | `\|\|` |

Arithmetic uses matching numeric types. Use `types:Convert` to change types explicitly. Integer division returns a float if the quotient is exactly representable. Thus `5 / 2` produces 2.5, while `1 / 3` raises a runtime error. Division by zero and arithmetic overflow raise runtime errors. Use `<` and `>` for ordering. Power is right associative. Boolean operators short-circuit, so `false && Expression()` never evaluates the right operand.

## Lists and strings

For a complete queue update, loop snapshot and delimited-record example, see [lists and Unicode text](Collections-and-Text.md).

Use `ls<T>` in declarations and `list<T>{...}` in expressions. Lists may have union element types, such as `ls<int | str>`. Read or replace a list element by its one-based `int` index. Indices must be within bounds. An `Insert` index can also be one past the end. `Insert` and `Remove` return updated lists, so assign their results if you want the variable to hold the updated list.

Strings can also be read with one-based indexing. It returns one Unicode scalar value as a string. A displayed character may comprise several scalar values. String positions are read-only.

Interpolation uses `#{expression}` inside double quotes. It can contain quoted strings and nested interpolation. Keep each interpolated expression on one physical line. Use `\n` for a newline, `\t` for a tab and `\"` for a quote inside string text.

```epholang
import "strings" as strings

ls<str> names := list<str>{"Ada", "Lin"}
names = Insert(names, 3, "Grace")
names[2] = "Linus"
Output("#{strings:ToUpperCase("#{names[2]}")}\n")
Output("café"[4])
```

## Functions and results

For a search that can return either a position or `null`, see [designing functions and using their results](Functions-and-Results.md).

Define functions at the top level with typed parameters. A function can call another user function that was declared earlier. Default arguments, direct recursion and indirect recursion are rejected.

Use `-> T` for a function returning a value. Every path must execute a typed `return T expression`. A function declared `-> none` still returns a value, such as `return none null`. Use `-|` for a procedure that finishes at the end of its body. It produces no value and rejects every form of `return`.

Consume a value through a declaration, assignment, argument, condition, return or another expression. If you only want its effects, write `_ = expression`. The right side executes once and any runtime error propagates. The discard creates no binding. A standalone literal, variable reference, arithmetic expression or value-returning call raises a static error.

```epholang
function Report() -> none:
	Output("Reported\n")
	return none null

function Finish() -|:
	Output("Done\n")

none result := Report()
Output("#{result}\n")
_ = Report()
Finish()
```

Here `Report()` produces `null` each time and writes a message. `Finish()` is already a complete statement, just like `Output(...)`.

## Control flow

For worked examples of short-circuit guards, search and backwards traversal, see [choosing branches and loops](Control-Flow.md).

Conditions must have type `bool`. Use `if`, optional `else if` branches and an optional `else`. A condition loop evaluates its condition before each iteration. A list loop visits the elements in order. `skip` advances the nearest enclosing loop and `break` exits it.

```epholang
ls<int> values := list<int>{1, 2, 3, 4}
loop value in values:
	if value == 2:
		skip
	else if value > 3:
		break
	Output("#{value}\n")
```

## Diagnostics

See [diagnose a program](Diagnostics.md) to reproduce a message and check a repair.

Use `epholang check file.epho` to catch errors before execution. Lexical and syntax errors include malformed literals, operators and indentation. Static errors include type mismatches, unused symbols, missing returns and discarded values without `_ =`. Runtime errors depend on execution and values, such as an out-of-bounds index computed from input or a failed conversion. The diagnostic shows the file and source position. For editor and formatter behavior, see [CLI and VS Code](CLI-Reference.md).
