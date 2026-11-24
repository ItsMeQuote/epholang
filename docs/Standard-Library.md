# Epholang 0.3 standard library

Built-in functions are available immediately. Import modules at the beginning of a file with `import "module" as alias`. Write a module call as `alias:Function(...)`, with no spaces around the colon. Aliases participate in the usual name and usage checks.

## Built-in functions

| Call | Result | Behavior |
| --- | --- | --- |
| `Output(value)` | No value | Prints the value with no automatic newline |
| `Input(prompt)` | `str` | Prints a string prompt, reads one line and removes its line ending |
| `Length(textOrList)` | `int` | Counts Unicode scalar values in a string or elements in a list |
| `Includes(container, value)` | `bool` | Tests substring containment or list membership. An empty string is a valid list element but an invalid substring pattern |
| `Insert(list, index, value)` | List | Returns a list with the value inserted at the one-based index. `Length(list) + 1` appends |
| `Remove(list, index)` | List | Returns the list with the indexed element removed |

```epholang
ls<str> labels := list<str>{"first", ""}
Output("#{Includes(labels, "")}\n")
labels = Insert(labels, Length(labels) + 1, "last")
labels = Remove(labels, 2)
Output("#{labels}\n")
```

## `strings`

| Function | Result | Arguments and behavior |
| --- | --- | --- |
| `Trim(text, pattern)` | `str` | Repeatedly removes a nonempty literal pattern from both ends |
| `Replace(text, old, new)` | `str` | Replaces occurrences of a nonempty substring. An empty replacement removes matches |
| `Split(text, separator)` | `ls<str>` | Splits on a nonempty separator and preserves empty fields |
| `StartsWith(text, prefix)`, `EndsWith(text, suffix)` | `bool` | Test the specified end of the string |
| `ToLowerCase(text)`, `ToUpperCase(text)` | `str` | Apply Unicode case conversion |
| `Capitalize(text)` | `str` | Uppercases the first alphabetic character and preserves the rest |
| `AlphabetRange(first, last)` | `str` | Returns an inclusive ascending or descending range. Endpoints must be single ASCII letters of the same case |
| `IsAsciiUppercase(text)`, `IsAsciiLowercase(text)` | `bool` | Require a nonempty string containing only ASCII letters of the requested case |
| `Repeat(text, count)` | `str` | Repeats text a nonnegative integer number of times. Zero yields an empty string |

```epholang
import "strings" as strings

Output("#{strings:Trim("--Beta--", "-")}\n")
Output("#{strings:AlphabetRange("D", "A")}\n")
Output("#{strings:IsAsciiLowercase("abc")}\n")
```

## `math`

| Function | Result | Arguments and behavior |
| --- | --- | --- |
| `AbsoluteValue(x)` | Same numeric type | Absolute value of an `int` or `flt`. The minimum signed integer overflows |
| `SquareRoot(x)` | `flt` | Square root of a nonnegative `flt` |
| `Floor(x)`, `Ceil(x)`, `Round(x)` | `flt` | Round down, up, or to nearest respectively. `Round` rounds ties away from zero |
| `Minimum(a, b)`, `Maximum(a, b)` | Same numeric type | Choose one of two values of matching numeric type |
| `Clamp(x, low, high)` | Same numeric type | Constrain the value to inclusive bounds. All three types must match and `low <= high` |
| `Sum(values)` | Element type | Sums an `ls<int>` or `ls<flt>`. An empty list sums to zero of its element type |
| `Average(values)` | `flt` | Takes the mean of a nonempty numeric list. Integer inputs require an exactly representable mean |
| `RandomInterval(low, high)` | `int` | Samples an integer between inclusive integer bounds. Intended for ordinary random sampling |

```epholang
import "math" as math

Output("#{math:Clamp(14, 0, 10)}\n")
Output("#{math:Average(list<int>{1, 2})}\n")
Output("#{math:RandomInterval(7, 7)}\n")
```

## `types`

| Function | Result | Behavior |
| --- | --- | --- |
| `Convert(value, target)` | Target type | Performs a supported conversion or reports an error |
| `IsConvertible(value, target)` | `bool` | Tests whether converting this value would succeed |
| `DeclaredTypeOf(variable)` | `type` | Inspect a variable's declared type, including unions |
| `CurrentTypeOf(expression)` | `type` | Evaluate the expression and inspect the value's actual type |

A conversion target must be a literal `int`, `flt`, `str`, `bool`, or `ls<str>`. In addition to identity conversions on the four scalar targets, supported pairs are as follows.

| From | To |
| --- | --- |
| `int` | `flt` if exact, or `str` |
| `flt` | `int` if integral and within range, or `str` |
| `str` | `int`, finite `flt`, `bool` for exactly `"true"` or `"false"`, or `ls<str>` with one string per Unicode scalar value |
| `bool` | `str` |

```epholang
import "types" as types

str input := "42"
if types:IsConvertible(input, int):
	int result := types:Convert(input, int)
	Output("#{result + 1}\n")
Output("#{types:Convert("café", ls<str>)}\n")
```
