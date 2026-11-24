# Declared types, current values and scope

## Choose the type at the point of declaration

A declaration such as `int attempts := 1` fixes what may be assigned to the variable for the rest of its scope. Use `=` for a later assignment. Redeclaring it with `:=` introduces a name a second time and is rejected. When a value can genuinely have different types over its lifetime, write the alternatives in the declaration.

```epholang
import "types" as t

int | none selected := null
Output("#{t:DeclaredTypeOf(selected)} / #{t:CurrentTypeOf(selected)}\n")
selected = 4
Output("#{t:DeclaredTypeOf(selected)} / #{t:CurrentTypeOf(selected)}\n")
```

```text
int | none / none
int | none / int
```

`null` is a value. It is useful for a meaningful absence, such as an unselected position. It belongs to `none`, so an `int` declaration alone cannot hold it. A procedure such as `Output` completes without any value to store, even in a variable whose type includes `none`.

## Match both sides of a function call

A parameter accepts values covered by its declared type. A return annotation records the contract offered back to the caller. A wide contract is useful when the caller only needs an operation that works for all members, such as rendering a value as part of a string.

```epholang
function Label(int | str id) -> str:
	return str "record #{id}"

Output("#{Label(7)}\n")
Output("#{Label("draft")}\n")
```

```text
record 7
record draft
```

Both arguments satisfy the parameter type. `true` would fail static checking. Conversely, if a parameter expects only `int`, a variable declared `int | str` can include a type the function cannot accept. If the task is to parse an external string into a number, validate and convert the string at the input boundary. The [library examples](Library-Recipes.md) show the two calls in sequence.

## Keep a value across a branch

A name declared inside a branch belongs to that branch. Declare a result before the branch when later code needs it. An inner scope also rejects a new declaration that reuses an existing outer variable's name.

```epholang
int score := 18
str label := "ordinary"
if score > 10:
	int bonus := score - 10
	label = "bonus #{bonus}"
Output("#{label}\n")
```

```text
bonus 8
```

`label` is visible both where it is assigned and where it is printed. `bonus` is visible only inside the indented body. Writing `str label := ...` inside the branch would try to declare a second variable with the same name. The single assignment updates the value that the outer statement reads.

## Decide when to use a union in collections

| Data to represent | Suitable declaration | Example assignment |
| --- | --- | --- |
| One position that may be absent | `int \| none` | `selected = null` |
| A sequence in which identifiers and labels can alternate | `ls<int \| str>` | `row[1] = "header"` if position 1 exists |
| Either an entire list of integers or an entire list of strings | `ls<int> \| ls<str>` | `column = list<str>{"header"}` |

For a mixed list, every stored element must belong to its declared element union. For a union of lists, a replacement list must match one of the allowed list types. The difference shows up during indexed assignment. The [collection examples](Collections-and-Text.md) walk through both cases.
