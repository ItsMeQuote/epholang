# Working with lists and Unicode text

## Replace, insert and remove

In `ls<str>` the type describes a variable. In `list<str>{}` it supplies the element type for a literal, even when the list is empty. Indexed assignment replaces an existing element. `Insert` and `Remove` return lists, so you choose which variable receives the result.

```epholang
ls<str> queue := list<str>{"build", "test"}
ls<str> original := queue
queue[1] = "compile"
queue = Insert(queue, Length(queue) + 1, "package")
queue = Remove(queue, 2)
Output("Original: #{original}\n")
Output("Current: #{queue}\n")
```

```text
Original: list{build, test}
Current: list{compile, package}
```

| Operation on a list of length N | Valid positions | Empty list |
| --- | --- | --- |
| Read or replace | 1 through N | No valid position |
| Remove | 1 through N | No valid position |
| Insert | 1 through N + 1 | Insert at 1 |

Removing position 2 shifts later elements left by one. To inspect the last element, guard the access with `Length(queue) > 0` and use `queue[Length(queue)]`. To test membership without finding a position, use `Includes(queue, "package")`.

## Choose what can vary in a list

`ls<int | str>` lets individual elements vary. `ls<int> | ls<str>` lets the variable hold a whole list of either declared kind. This distinction affects which writes are safe.

```epholang
ls<int | str> row := list<int | str>{12, "ready"}
row[2] = 13
Output("#{row}\n")

ls<int> | ls<str> column := list<int>{12, 13}
Output("#{Length(column)} entries\n")
column = list<str>{"ready"}
Output("#{column[1]}\n")
```

```text
list{12, 13}
2 entries
ready
```

Writing an integer to `row[2]` is valid because `int` belongs to the element union. A write such as `column[1] = "ready"` would have to work for an integer list as well. Assigning a complete `list<str>` to `column` selects the other allowed list type. List elements in Beta are scalars. To represent a second sequence, declare another list.

## Understand iteration during updates

A list loop iterates the elements present when it starts. The body can assign a new list to the source variable without extending the current traversal.

```epholang
ls<int> numbers := list<int>{1, 2}
loop number in numbers:
	numbers = Insert(numbers, Length(numbers) + 1, number * 10)
Output("#{numbers}\n")
```

```text
list{1, 2, 10, 20}
```

Only 1 and 2 are visited. When an algorithm needs to visit newly added elements, use a position variable and re-evaluate the current length in a condition loop. Make sure the loop has a stopping condition.

## Count, index and split text

String indexing uses the same one-based positions. A position refers to a Unicode scalar value. UTF-8 byte length and the number of glyphs drawn by a font can differ from `Length`. A combining mark, for example, occupies its own position. Converting to `ls<str>` exposes each scalar as a separate string.

```epholang
import "strings" as s
import "types" as t

str word := "café"
Output("#{Length(word)}: #{word[4]}\n")
Output("#{t:Convert(word, ls<str>)}\n")
ls<str> fields := s:Split("red||blue|", "|")
Output("#{Length(fields)} fields, second length #{Length(fields[2])}\n")
Output("#{Includes(fields, "")}\n")
```

```text
4: é
list{c, a, f, é}
4 fields, second length 0
true
```

Consecutive separators create an empty field. A trailing separator creates another. This matters when field positions have meaning. `Includes(fields, "")` checks for an empty element. Substring search on a string requires a nonempty pattern. See the [standard library](Standard-Library.md) for trimming, replacing and case conversion.
