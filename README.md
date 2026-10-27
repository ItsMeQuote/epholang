<p align="center">
  <img src="images/epholang_logo.png" alt="Epholang logo" width="auto">
</p>

<div align="center">
  <h1>Epholang</h1>
</div>

<p align="center">
  An experimental statically checked programming language.
</p>

---

Epholang is a programming language built around a few simple ideas:

- **One way to do a thing.** Fewer overlapping features means fewer choices that don't matter.
- **Explicit over short.** Code doesn't have to be brief. It has to be easy to read.
- **Strict, not forgiving.** Unused variables, missing returns and type mismatches are errors, not warnings.
- **Visible changes.** If data changes, you can see it in the code.
- **Tradition is not an argument.** Lists start at 1. Negative numbers are written as `(-1)`.
- **No classical OOP.** No classes, no inheritance, no methods.

> **Status:** early and experimental (`0.1.0`).
> The current implementation is a tree-walking interpreter with a full static checking pass.
> Syntax and behavior may change in incompatible ways. A native compiler is planned.

## A taste of Epholang

```epholang
greet func := function(name str = "") str {
    if name == "" {
        return str "Hello!"
    } otherwise {
        return str "Hello, #{name}!"
    }
}

output("#{greet()}\n")
output("#{greet(\"World\")}\n")
```

```text
Hello!
Hello, World!
```

---

```epholang
nums ls<num> := list<num>{10, 20, 30}
nums = insert(nums, 1, 5)
nums = remove(nums, 2)

total num := 0
cycle n in nums {
    total = total + n
}

output("#{nums} has #{length(nums)} items, sum = #{total}\n")
```

```text
list{5, 20, 30} has 3 items, sum = 55
```

## Errors that point at the problem

All static errors are reported at once, **before** the program starts:

```epholang
name str := "Epholang"
name = 2026
output(name)
```

```text
Static error: Cannot assign num to 'name' of type str
 --> example.epho:2:8
  |
2 | name = 2026
  |        ^^^^
```

There are three kinds of errors:

|      Kind       |          When          |   How many    |
| :-------------: | :--------------------: | :-----------: |
| `Syntax error`  | while reading the code | the first one |
| `Static error`  |     before running     |  all of them  |
| `Runtime error` |     while running      | the first one |

## Getting started

### Download

Prebuilt Windows binaries are available on the [Releases](../../releases) page.

### Build from source

You need a stable [Rust](https://www.rust-lang.org/tools/install) toolchain.

```sh
git clone https://github.com/ItsMeQuote/epholang.git
cd epholang
cargo build --release
```

### Run

```sh
epholang run program.epho      # check and run
epholang check program.epho    # static check only
epholang tokens program.epho   # show lexer output
epholang parse program.epho    # show the syntax tree
```

Tested on Windows. Other platforms should work, but haven't been verified yet.

## Language overview

### Types

`num`, `str`, `bool`, `none`, `type` and lists `ls<T>`, where `T` is `num`, `str`, `bool` or `none`.

### Variables

```epholang
age num := 17
age = 18             /* the type can't change */
```

Variables live in the block where they are declared. Shadowing an outer name is an error.

### Functions

```epholang
square func := function(x num) num {
    return num x * x
}

log func := function(message str) none {
    output("[log] #{message}\n")
    return none null
}
```

- Every `return` states its type.
- A function must return on every path.
- Parameters may have default values.
- Functions are declared at the top level, before they are used.
- A function cannot assign to global variables.

### Conditions and loops

```epholang
if x > 10 {
    output("big\n")
} otherwise if x > 5 {
    output("medium\n")
} otherwise {
    output("small\n")
}

i num := 1
cycle i < 4 {
    output("#{i}\n")
    i = i + 1
}

cycle item in items {
    output("#{item}\n")
}
```

Conditions must be `bool`. There are no `<=` and `>=`: write `i < 4` or `!(a > b)`.

### Lists

```epholang
xs ls<str> := list<str>{"a", "b"}
output(xs[1])               /* first element */
output(xs[length(xs)])      /* last element */
xs[1] = "z"
xs = insert(xs, 2, "c")     /* returns a new list */
xs = remove(xs, 1)          /* returns a new list */
```

- Indexes start at 1. Index `0`, negative and fractional indexes are errors.
- Assigning a list makes an independent copy.
- `==` compares type, length and elements in order.

### Strings

```epholang
output("Hello, #{name}!\n")    /* interpolation: any expression */
output("Literal: \#{name}\n")  /* escaped */
output(length("Hello!"))       /* 6, counts characters */
```

`output` never adds a newline on its own. Use `\n`. Strings are not joined with `+`; use interpolation instead.

### Numbers

```epholang
x num := (-1)          /* unary minus only inside parentheses */
output(2 ^ 3 ^ 2)      /* power, right-associative: 512 */
output(7 % 3)
```

### Comments

There is only one kind of comment, and it can be nested:

```epholang
/* outer
	/* inner */
	still a comment
*/
```

### Built-in functions

|           Function           |                      Description                       |
| :--------------------------: | :----------------------------------------------------: |
|       `output(value)`        |                     prints a value                     |
|         `length(x)`          | number of characters in a string or elements in a list |
| `insert(list, index, value)` |            new list with the value inserted            |
|    `remove(list, index)`     |              new list without the element              |
|       `typeOf(value)`        |          the type of a value, e.g. `ls<num>`           |

## Not there yet

- input from the user
- union types and nested lists
- error handling as values
- `datatable`
- a native compiler

## About this project

Epholang's implementation was written with the help of AI.

The language itself was designed by a human: the syntax, the semantics, the rules and every design decision come from [ItsMeQuote](https://github.com/ItsMeQuote).

## Community

News and development updates (in Russian): [t.me/epholang](https://t.me/epholang)

## License

[MIT](LICENSE)
