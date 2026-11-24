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

## Migrate return values by intent

| Old call site | Intended behavior | 0.3 form |
| --- | --- | --- |
| A call returns a number or string that will be used | Keep the value | Assign it to a correctly typed variable or pass it as an argument |
| A call returns a value but only its side effects matter | Evaluate and discard | Write _ = Calculate() |
| A function exists only to write output | Execute a procedure | Declare it with -| and call it by itself |
| A function returns null for use by the caller | Keep the actual null | Declare it with -> none and use return none null |

If an old function used Output as a return expression, move Output onto its own line. Add a separate typed return if the caller expects a value. If old code assigned to a variable named exactly _, rename that variable. Names such as _count remain normal identifiers and still need references.

## Migrate a type inspection

```epholang
import "types" as types

int | str value := 7
Output("#{types:DeclaredTypeOf(value)}\n")
value = "ready"
Output("#{types:CurrentTypeOf(value)}\n")
```

The first line of output is the declared union, int | str. The second line is str because the latest assigned value is a string.

## Verify the migrated file

1. Fix parser diagnostics first. Check block indentation, the colon at the end of each header, and declaration order.
2. Run the static checker. Review unused declarations and discarded values before deleting any code with side effects.
3. Run the formatter in write mode once, then use its check mode in CI.
4. Run the program with representative inputs. Verify output, conversions and boundary cases such as first and last list indices.
