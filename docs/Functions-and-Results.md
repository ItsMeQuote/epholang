# Designing functions and using their results

## Give callers a contract

Suppose a program searches for a name. A match has a one-based position. A miss is meaningful to the caller, so the result type can express both outcomes. Declare the function before its first call, and put the return type on each return statement.

```epholang
function FindName(ls<str> names, str wanted) -> int | none:
	int index := 1
	loop name in names:
		if name == wanted:
			return int index
		index = index + 1
	return none null

ls<str> names := list<str>{"Ada", "Lin"}
Output("Ada: #{FindName(names, "Ada")}\n")
Output("Sam: #{FindName(names, "Sam")}\n")
```

```text
Ada: 1
Sam: null
```

The counter advances only after an unsuccessful comparison. A match returns immediately from the function, even from inside the loop. The final return covers both an empty list and a search that inspected every element. Its value has type `none`, a member of the declared union. If a caller stores the result for later, the receiving declaration must accept either outcome.

## Put output at the call site or in a procedure

A calculation can serve callers that want different output. Here `Total` computes a result, and `PrintTotal` decides how to present it. The output procedure can finish naturally after writing its line.

```epholang
function Total(int count, int price) -> int:
	return int count * price

function PrintTotal(int count, int price) -|:
	Output("#{count} items cost #{Total(count, price)}\n")

PrintTotal(3, 7)
Output("Two orders cost #{Total(3, 7) * 2}\n")
```

```text
3 items cost 21
Two orders cost 42
```

A procedure call is a statement. Putting `PrintTotal(3, 7)` inside interpolation, an assignment or a return expression asks the checker for a value the procedure cannot provide. An early `return` is also invalid in a procedure. To make an effect conditional, put the effect in an `if` body.

## Discard a value and keep its effects

The right side of `_ = expression` is checked and evaluated once. Discard works for any value-producing expression, including arithmetic and `null`. It introduces no name that you can read later.

```epholang
function ReadSensor() -> int:
	Output("Reading sensor\n")
	return int 12

_ = ReadSensor() + 10
_ = null
Output("Done\n")
```

```text
Reading sensor
Done
```

The computation still calls `ReadSensor` and adds ten. Discarding a division or an index operation also preserves its runtime errors. If the program needs the number later, bind it at the call site or pass it directly to the next calculation.

## Check a declaration and its call together

| What you see | What to inspect | Repair |
| --- | --- | --- |
| A function may finish without returning | Walk through each branch and the path past a loop that executes zero times | Return an allowed value on every path |
| A call produces no value to discard | The callee uses `-\|` or is `Output` | Call it on its own line |
| An expression value is unused | The callee returns a value, including `none` | Consume the result or explicitly discard it |
| A name cannot be resolved | Look for a call before the declaration or a reference outside its scope | Move the declaration before its caller or pass the value as a parameter |
| A parameter or function is unused | Look for a declaration that no code references | Remove the unused part or connect it to its intended caller |

User functions are top-level declarations. Define dependencies before the functions that use them. Beta rejects recursive calls, including a cycle through another function. For repetition, use a [loop](Control-Flow.md). For examples of the actual command-line messages, see [diagnostics](Diagnostics.md).
