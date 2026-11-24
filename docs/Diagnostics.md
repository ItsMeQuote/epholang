# Diagnose an Epholang program

Save each example in a separate UTF-8 file named `case.epho`. Run `epholang check case.epho` for syntax and static errors, then `epholang run case.epho` for behavior with actual values. The excerpts below quote the message text. The CLI also points to the affected source location.

## A result was left on its own line

```epholang
function Double(int value) -> int:
	return int value * 2

Double(5) + 1
```

**Check fails** with `Unused expression value; use '_ = expression' to discard it`. The entire last line computes 11 and leaves it unused. `Output(Double(5) + 1)` prints it. `_ = Double(5) + 1` evaluates the same expression and discards it. Choose the fix according to what the caller needs.

## A procedure was used as a value

```epholang
_ = Output("ready")
```

**Check fails** with `This call produces no value to discard`. Use `Output("ready")` on its own line. Because `run` checks the file first, the invalid program never prints its message. If another function needs to supply text for printing, let that function return a `str` and pass its result to `Output`.

## A function can finish without a result

```epholang
function Label(int score) -> str:
	if score > 0:
		return str "positive"

Output(Label(2))
```

**Check fails** with `Function 'Label' does not return a value on every path`. Although this caller supplies 2, the declaration permits 0 and negative numbers. Add a final `return str "nonpositive"` at the function body's indentation level. A loop that returns from its body can also leave a path uncovered when the loop never starts.

## A correctly typed index still fails at runtime

```epholang
ls<str> cities := list<str>{"Oslo", "Lima"}
int position := Length(cities) + 1
Output(cities[position])
```

**Check passes. Run fails** with `Index 3 out of bounds (max 2)`. Position 3 is an integer, so its type is appropriate. The current list has only two entries. Use `Length(cities)` for the last entry after checking the list is nonempty. For a position supplied by a user, validate both ends before indexing.

```epholang
ls<str> cities := list<str>{"Oslo", "Lima"}
int position := 3
if position > 0 && position < Length(cities) + 1:
	Output("#{cities[position]}\n")
else:
	Output("Choose a position from 1 to #{Length(cities)}\n")
```

```text
Choose a position from 1 to 2
```

## A division loses precision

```epholang
Output(1 / 3)
```

**Check passes. Run fails** with `Integer division would lose floating-point precision`. One third has no exact binary floating-point representation. If rounding is acceptable, use `1.0 / 3.0`. If the exact fraction matters, keep the integer numerator and denominator as separate values. A zero divisor is a separate runtime error.

## Read indentation and name errors

| Symptom | First thing to inspect | Correction |
| --- | --- | --- |
| `Indentation must use tabs` | Show whitespace on the line marked by the CLI | Replace indentation spaces with one literal tab per block level |
| `Indentation must increase by one tab` | Compare the header and its first statement | Add one level at a time. A nested block needs its own header |
| An identifier cannot be found after an `if` | Check where the variable was declared | Declare it outside the branch if later statements need it |
| A name is already declared in an outer scope | Look for a parameter or a local with the same name as an enclosing variable | Reuse the existing variable through assignment or give the new one its own name |
| An unused name begins with `_` | Inspect the declaration and its references | Remove an unnecessary declaration or use it. Only a lone `_` on the left of a discard has special meaning |

If the structure still looks correct, use `epholang tokens case.epho` to inspect indentation tokens and `epholang parse case.epho` to inspect the statement structure. Once the file parses, run `check` again. EphoFMT can reformat valid syntax. Static type checks and execution remain separate steps. See [CLI commands](CLI-Reference.md) and [functions and results](Functions-and-Results.md).
