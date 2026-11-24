# Standard library in practice

The [function reference](Standard-Library.md) lists signatures and accepted arguments. Each program below can be saved in its own file and run independently. Put module imports above declarations and refer to each module by its local alias.

## Clean a delimited line without losing a column

Suppose a line contains a header, a missing middle field and a status ending in a newline. Trim the record terminator, split on the field separator, then normalize individual fields. Removing separators or blanks before splitting can change which value belongs to each column.

```epholang
import "strings" as s

str record := " ID || Ready \n"
ls<str> fields := s:Split(s:Trim(record, "\n"), "|")
if Length(fields) == 3:
	str key := s:ToLowerCase(s:Trim(fields[1], " "))
	str status := s:ToUpperCase(s:Trim(fields[3], " "))
	Output("#{key}: #{status}, middle length #{Length(fields[2])}\n")
else:
	Output("Expected three fields\n")
```

```text
id: READY, middle length 0
```

`Trim` removes the literal pattern you supply at the ends. A space and a newline are different patterns. The length check makes both indexed reads safe. `Split` is a literal separator operation. A format with quoted or escaped separators needs additional parsing.

## Validate a number before calculating with it

Use `types:IsConvertible` when the input may be malformed. Pass the same value and target to `types:Convert` after a successful check. Here one invalid field is skipped and the two valid readings are clamped to inclusive bounds before they reach `Sum`.

```epholang
import "types" as t
import "math" as m

ls<int> accepted := list<int>{}
loop field in list<str>{"12", "missing", "105"}:
	if t:IsConvertible(field, int):
		int reading := t:Convert(field, int)
		accepted = Insert(accepted, Length(accepted) + 1, m:Clamp(reading, 0, 100))
	else:
		Output("Skipped #{field}\n")
Output("Sum: #{m:Sum(accepted)}\n")
if Length(accepted) > 0:
	Output("Mean: #{m:Average(accepted)}\n")
```

```text
Skipped missing
Sum: 112
Mean: 56
```

The stored readings are 12 and 100. `Sum` accepts an empty numeric list. `Average` requires at least one element, so the length guard matters if all fields fail conversion. An integer mean must also be exactly representable as a float. For example, the mean of 1, 2 and 4 is 7/3 and fails that check.

## Choose exact or floating-point arithmetic

Integer division produces a float only when the quotient is exactly representable. `5 / 2` is exact in binary floating point. `1 / 3` would require rounding and produces a runtime diagnostic. When an approximation is appropriate, choose floating-point operands explicitly before dividing.

```epholang
import "types" as t
import "math" as m

int count := 3
flt divisor := t:Convert(count, flt)
flt share := 1.0 / divisor
Output("#{5 / 2}\n")
Output("Between zero and one: #{share > 0.0 && share < 1.0}\n")
Output("Rounded percent: #{m:Round(share * 100.0)}\n")
```

```text
2.5
Between zero and one: true
Rounded percent: 33
```

Converting after integer division is too late. The interpreter evaluates a function argument before the function can use it. For a large integer input, check whether conversion to `flt` preserves its value with `IsConvertible`.

## Distinguish declared type, current type and conversion

Use `DeclaredTypeOf` to see the set of values a variable may hold. Use `CurrentTypeOf` to inspect the value at a particular moment. `IsConvertible` answers a third question about that value's contents.

```epholang
import "types" as t

str | int token := "007"
Output("declared #{t:DeclaredTypeOf(token)}, current #{t:CurrentTypeOf(token)}\n")
Output("can parse: #{t:IsConvertible("007", int)}\n")
token = 7
Output("declared #{t:DeclaredTypeOf(token)}, current #{t:CurrentTypeOf(token)}\n")
```

```text
declared str | int, current str
can parse: true
declared str | int, current int
```

Printing either type leaves the variable as it was. Assigning an integer on the following line changes its current value. When input has a spelling you must preserve, such as an identifier with leading zeros, keep the original string separately from its numeric interpretation.
