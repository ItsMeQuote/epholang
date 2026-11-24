# Write your first Epholang program

This guide uses Epholang 0.3 Beta. Follow the [installation guide](Installation.md) to obtain the interpreter and verify its archive.

## Start with output

Save this as `hello.epho` in UTF-8.

```epholang
Output("Hello, Epholang!\n")
```

Run `epholang check hello.epho`, then `epholang run hello.epho`. The first command checks the program. The second checks it again and prints `Hello, Epholang!`. The explicit `\n` moves the cursor to the next line.

## Work with a typed list

Save the program below as `scores.epho`. Use literal tabs where the code is indented.

```epholang
import "math" as math

function Grade(int score) -> str:
	if score > 89:
		return str "excellent"
	else if score > 69:
		return str "pass"
	else:
		return str "retry"

function PrintScore(int score) -|:
	Output("#{score}: #{Grade(score)}\n")

ls<int> scores := list<int>{100, 60, 80}
loop score in scores:
	PrintScore(score)

Output("Average: #{math:Average(scores)}\n")
```

```text
100: excellent
60: retry
80: pass
Average: 80
```

The import makes the math module available as `math`. `Grade` always returns a string. `PrintScore` writes output and finishes without a value. `loop score in scores` repeats the body for each item. Each interpolation consumes its expression, including the result of `math:Average`.

## Read a value and handle failed conversion

```epholang
import "types" as types

str answer := Input("Enter a whole number: ")
if types:IsConvertible(answer, int):
	int number := types:Convert(answer, int)
	Output("You entered #{number}\n")
else:
	Output("Expected a whole number\n")
```

The prompt and response are strings. Check convertibility before asking for an integer. If you intentionally want only the side effects of a call that returns a value, write an explicit discard, for example `_ = Input("Press Enter to continue: ")`.

## Diagnose, format and explore

Try putting a standalone `Grade(80)` at the end of `scores.epho`. `check` reports an unused result. Replace it with `Output(Grade(80))` to print the result or `_ = Grade(80)` to discard it.

Use `epholang format scores.epho` to format the file. Use `format --check` to check formatting without writing. For another runnable program, choose a file from [examples](../examples/). Search the [language reference](Language-Reference.md) for operators, declarations and loops, or the [standard library](Standard-Library.md) for function contracts.
