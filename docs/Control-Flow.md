# Choosing branches and loops

## Classify a value once

An `if` chain tests conditions from top to bottom. The first true condition selects the body and ends the chain. Ordering matters when two conditions can match the same value. Each branch has its own scope. In this program the function returns from each branch, so the caller receives a string for every possible temperature.

```epholang
function Describe(int temperature) -> str:
	if temperature < 0:
		return str "freezing"
	else if temperature < 20:
		return str "cool"
	else:
		return str "warm"

loop reading in list<int>{(-1), 0, 19, 20}:
	Output("#{reading} #{Describe(reading)}\n")
```

```text
-1 freezing
0 cool
19 cool
20 warm
```

Zero reaches the second test. Twenty reaches the final branch. If you need a value outside a branch, either return it as above or declare a variable before the chain and assign to it in the selected branch.

## Search until you find a match

Use `loop item in items` to visit list elements in order. `skip` moves directly to the next element. `break` leaves the nearest enclosing loop and continues after its body. The following search ignores blanks and stops after the first match.

```epholang
ls<str> names := list<str>{"", "Ada", "Lin", "Lin"}
int examined := 0
loop name in names:
	if name == "":
		skip
	examined = examined + 1
	if name == "Lin":
		Output("Found after #{examined} names\n")
		break
```

```text
Found after 2 names
```

The blank never increments the counter. The second occurrence of Lin is never visited. An empty list executes the body zero times. When a function searches a list and promises a result, provide a return after the loop for the empty and no-match cases.

## Advance a position in a condition loop

Use `loop condition` when you need to control the position yourself. The condition runs before every iteration, including the first. Here the position counts down, so the same code also handles an empty string.

```epholang
str text := "café"
int position := Length(text)
loop position > 0:
	Output(text[position])
	position = position - 1
Output("\n")
```

```text
éfac
```

The last valid position is `Length(text)` and the first is 1. The update must run on every path that continues the loop. A `skip` placed before the decrement would repeatedly visit the same position.

## Guard a read with a Boolean expression

`&&` evaluates its right operand only if the left operand is true. `||` evaluates its right operand only if the left is false. Both operands must still pass static checking. Put the precondition before the expression that depends on it.

```epholang
function HasHeader(ls<str> lines) -> bool:
	return bool Length(lines) > 0 && lines[1] == "name"

Output("#{HasHeader(list<str>{})}\n")
Output("#{HasHeader(list<str>{"name", "Ada"})}\n")
```

```text
false
true
```

The first call returns before attempting the index. Reversing the tests would try to read position 1 from an empty list. For a loop that changes its source list, see [lists and text](Collections-and-Text.md).
