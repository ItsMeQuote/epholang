# Epholang Documentation

Epholang is an experimental programming language focused on explicitness, predictable behavior, and static checking.

- **Current version:** 0.1.0
- **Implementation:** Rust
- **Execution model:** tree-walking interpreter with a static checking pass

Epholang is under active development. Syntax, semantics, and available features may change between versions.

## Design Principles

- **One way to do a thing.** Avoid unnecessary overlapping features.
- **Explicit over short.** Prefer readable code over minimal syntax.
- **Strict static checking.** Detect type mismatches, unused variables, and missing returns before execution.
- **Visible changes.** Make data modifications apparent in the code.
- **No classical OOP.** No classes, inheritance, or methods.
- **One-based list indexing.** The first element has index `1`.

## Language Features

### Types

The current language includes the following types:

- `num` — numbers
- `str` — strings
- `bool` — Boolean values
- `none` — absence of an ordinary result
- `type` — type values
- `ls<T>` — lists of supported element types

Supported list element types include `num`, `str`, `bool`, and `none`.

### Variables

Declare a variable with `:=`:

```epholang
age num := 17
```

Assign a new value with `=`:

```epholang
age = 18
```

A variable's type cannot change after declaration.

### Functions

Functions use the `function` keyword and declare their return type:

```epholang
square func := function(x num) num {
    return num x * x
}
```

Functions can accept parameters with default values and must return values according to their declared return types.

### Conditions and loops

Use `if`, `otherwise if`, and `otherwise` for conditional execution.

The `cycle` construct supports conditional loops and iteration over collections.

### Lists

Lists use the `ls<T>` type notation and `list<T>{...}` syntax for construction.

Indexes start at `1`. The built-in `insert` and `remove` functions return modified lists.

### Strings

Strings use double quotes. Interpolation uses `#{...}`:

```epholang
name str := "Epholang"
output("Hello, #{name}!\n")
```

Use `\n` to insert a newline. The `output` function does not append one automatically.

## Command-Line Interface

The interpreter supports four commands:

```sh
epholang run <file.epho>     # `run` performs static checking and executes the program.
epholang check <file.epho>   # `check` performs static checking without execution.
epholang tokens <file.epho>  # `tokens` displays lexer output.
epholang parse <file.epho>   # `parse` displays the parsed syntax tree.
```

## Examples

Runnable examples are available in the [`examples/`](../examples/) directory:

- [`hello_world.epho`](../examples/hello_world.epho)
- [`functions.epho`](../examples/functions.epho)
- [`lists.epho`](../examples/lists.epho)
- [`loops.epho`](../examples/loops.epho)

## Current Limitations

Epholang 0.1.0 does not yet include all planned features.

Current limitations include:

- User input
- Union types and nested lists
- Error handling as values
- `datatable`
- Native compilation

## Project Status

Epholang is an experimental language. Some implementation details and language rules may evolve as development continues.

The language was designed by ItsMeQuote, with AI assistance in implementing the compiler/interpreter.

See the [README](../README.md) for an overview and instructions for building the project.
