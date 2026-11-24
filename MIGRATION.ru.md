# Переход с Epholang 0.2 на 0.3 (β)

В объявлениях тип ставится перед именем. Например, `value int := 7` становится `int value := 7`. Заголовок `function Square(x int) int {` заменяется на `function Square(int x) -> int:`. Уберите закрывающие фигурные скобки блоков, используйте один таб на уровень и замените `otherwise if` и `otherwise` на `else if` и `else`.

```epholang
function Square(int x) -> int:
	return int x * x

function Display(int result) -|:
	Output(result)

Display(Square(7))
```

Сохраняйте `-> none` и `return none null`, когда вызывающему коду нужно значение null. Для процедуры укажите `-|` и уберите return. У Output больше нет результата. Для намеренного отбрасывания значения пишите `_ = Square(7)`. Одиночный знак подчёркивания допустим только слева в этой инструкции. Вместо TypeOf импортируйте модуль types и используйте types:CurrentTypeOf.

Установите CLI и расширение одной версии. После исправления синтаксиса выполните `epholang check program.epho`, `epholang format --check program.epho` и `epholang run program.epho`.
