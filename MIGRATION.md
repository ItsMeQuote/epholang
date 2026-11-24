# Migrating from Epholang 0.2 to 0.3 (β)

| 0.2 | 0.3 |
| --- | --- |
| `value int := 7` | `int value := 7` |
| `function Square(x int) int {` | `function Square(int x) -> int:` |
| Braces around a block | Colon and one literal tab per level |
| `otherwise if` and `otherwise` | `else if` and `else` |
| A standalone value-returning call | Use its result or write `_ = expression` |
| `TypeOf(value)` | Import `types` and call `types:CurrentTypeOf(value)` |

**0.2**

```epholang
function Square(x int) int {
    return int x * x
}
Output(Square(7))
```

**0.3**

```epholang
function Square(int x) -> int:
	return int x * x

function Display(int result) -|:
	Output(result)

Display(Square(7))
```

Keep `-> none` and `return none null` when callers consume an actual null value. For a procedure, use `-|` and remove its return statement. Output now produces no value. A standalone expression such as `42` or `Square(7) + 1` needs an explicit discard if its value is unwanted. A lone underscore is reserved for the left side of discard.

Use a matching CLI and VSIX. Run `epholang check program.epho`, then `epholang format --check program.epho` and `epholang run program.epho`. EphoFMT accepts syntactically valid 0.3 source after you migrate its block structure.
