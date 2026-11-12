<p align="center">
  <img src="images/epholang_logo_icon.png" alt="Логотип Epholang" width="auto">
  <img style="width: 5%; height: auto;">
  <img src="images/epholang_hexer.png" alt="Маскот Хексер">
</p>

<div align="center">
  <h1>Epholang</h1>
</div>

<p align="center">
  Экспериментальный язык программирования, в котором важны <strong>явность, читаемость и строгая статическая проверка</strong>.
</p>

<p align="center">
  <a href="https://github.com/ItsMeQuote/epholang/releases"><img src="https://img.shields.io/badge/версия-0.2.0%20(Alpha)-blue" alt="Версия"></a>
  <img src="https://img.shields.io/badge/реализация-Rust-orange" alt="Rust">
  <a href="https://github.com/ItsMeQuote/epholang/stargazers"><img src="https://img.shields.io/github/stars/ItsMeQuote/epholang" alt="Звёзды"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ItsMeQuote/epholang" alt="Лицензия"></a>
  <a href="https://github.com/ItsMeQuote/epholang/actions/workflows/ci.yml"><img src="https://github.com/ItsMeQuote/epholang/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
</p>

<p align="center"><a href="README.md">English</a> &bull; <strong>Русский</strong></p>

---

## Почему Epholang?

<img src="images/epholang_hexer_question.png" align="right" align="right" alt="Хексер со знаком вопроса">

В основе Epholang — несколько осознанных принципов:

- **Минимум способов на одну задачу.** Без лишних пересекающихся возможностей.
- **Явность важнее краткости.** Изменения данных и типов должны быть видны в коде.
- **Строгость вместо снисходительности.** Ошибки типов, неиспользуемые переменные и отсутствие `return` не замалчиваются.
- **Традиция — не аргумент.** Индексы списков начинаются с **1**, отрицательные числа записываются как `(-1)`.
- **Никакого классического ООП.** Нет классов, наследования и методов.

Сейчас Epholang — **интерпретатор с обходом синтаксического дерева и предварительной статической проверкой**. Язык экспериментальный: несовместимые изменения в следующих версиях возможны.

## Знакомство с Epholang

<img src="images/epholang_hexer_laptop.png" align="right" align="right" alt="Хексер держит ноутбук">

```epholang
import "math" as math

function Square(value int) int {
    return int value * value
}

numbers ls<int> := list<int>{1, 2, 3}
loop number in numbers {
    Output("#{number} в квадрате = #{Square(number)}\n")
}

Output("Сумма = #{math:Sum(numbers)}\n")
```

```text
1 в квадрате = 1
2 в квадрате = 4
3 в квадрате = 9
Сумма = 6
```

В языке есть явные типы (`int`, `flt`, `str`, `bool`), типизированные функции, интерполяция строк, списки с индексацией с единицы и встроенные модули. Имена функций рекомендуется писать в **PascalCase**, а имена переменных и псевдонимов модулей — в **camelCase**. Это _соглашение_, а не ограничение интерпретатора.

Семь готовых программ находятся в [`examples/`](examples/). Подробное руководство и справочник языка — в [GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki).

## Начало работы

<img src="images/epholang_hexer_arrow.png" align="right" align="right" alt="Хексер со стрелкой">

### Скачать или собрать

Готовый Windows-файл и расширение VS Code смотрите в [Releases](https://github.com/ItsMeQuote/epholang/releases). Для самостоятельной сборки установите стабильный набор инструментов [Rust](https://www.rust-lang.org/tools/install):

```sh
git clone https://github.com/ItsMeQuote/epholang.git
cd epholang
cargo build --release
```

Исполняемый файл будет находиться в `target/release/epholang` (Windows: `target\release\epholang.exe`). Для сборки конкретной версии используйте соответствующий тег с исходниками **0.2.0**.

### Командная строка

```sh
epholang run program.epho      # проверить и запустить
epholang check program.epho    # только статическая проверка
epholang tokens program.epho   # показать токены лексера
epholang parse program.epho    # показать синтаксическое дерево
```

Пример можно запустить через Cargo, не добавляя программу в `PATH`:

```sh
cargo run -- run examples/03_loops.epho
```

### VS Code

Установите `epholang-language-0.2.0.vsix` через **Extensions → … → Install from VSIX...** или командой `code --install-extension epholang-language-0.2.0.vsix` из папки с файлом. Расширение добавляет подсветку синтаксиса и сниппеты, но не является компилятором или языковым сервером.

## Краткий обзор языка

| Возможность | Пример или правило                                                        |
| ----------- | ------------------------------------------------------------------------- |
| Числа       | `int` (`i64`) и `flt` (`f64`), без неявного смешивания                    |
| Другие типы | `str`, `bool`, `none` и значения типов (`type`)                           |
| Списки      | `ls<int>`, `ls<flt>`, `ls<str>`, `ls<bool>`, `ls<none>` — без вложенности |
| Переменные  | `age int := 20` и `age = 21`                                              |
| Функции     | `function Square(x int) int { return int x * x }`                         |
| Условия     | `if ... { } otherwise if ... { } otherwise { }`                           |
| Циклы       | `loop условие { }` или `loop элемент in список { }`                       |
| Строки      | `Output("Привет, #{name}!\n")`                                            |
| Комментарии | `/* блочные комментарии /* можно вкладывать */ */`                        |

**Функции** требуют явных типов параметров и результата, типизированного `return` и возврата значения на всех путях. Параметров по умолчанию нет. Функции объявляются на верхнем уровне до использования.

**Списки** индексируются с **1**. Индексы должны иметь тип `int`. `Insert` и `Remove` возвращают новые списки. `Output()` и `Input()` не добавляют перевод строки автоматически — для этого нужен `\n`.

**Арифметика** различает `int` и `flt`. Выражение `int / int` возвращает `flt` только в том случае, если частное точно представимо в двоичном формате `f64`: `5 / 2` работает, а `1 / 3` выдаёт ошибку выполнения. Переполнение `int` — ошибка, смешивание числовых типов требует явного преобразования.

## Встроенные функции

| Функция                      | Назначение                                 |
| ---------------------------- | ------------------------------------------ |
| `Output(value)`              | Вывод без автоматического переноса строки  |
| `Input(prompt)`              | Чтение строки из консоли                   |
| `Length(value)`              | Число символов строки или элементов списка |
| `Includes(container, value)` | Проверка подстроки или элемента списка     |
| `Insert(list, index, value)` | Новый список со вставленным элементом      |
| `Remove(list, index)`        | Новый список без элемента                  |
| `TypeOf(value)`              | Получение типа значения                    |

## Стандартные модули

Модули импортируются в начале файла. Для вызова используется синтаксис `псевдоним:Функция(...)`:

```epholang
import "strings" as strings
import "math" as math
import "types" as types

Output(strings:ToUpperCase("hello"))
Output("\n")
Output("Случайное число: #{math:RandomInterval(1, 6)}\n")
Output("Можно преобразовать: #{types:IsConvertible(\"42\", int)}\n")
```

| Модуль    | Функции                                                                                                                    |
| --------- | -------------------------------------------------------------------------------------------------------------------------- |
| `strings` | `Trim`, `Replace`, `Split`, `StartsWith`, `EndsWith`, `ToLowerCase`, `ToUpperCase`, `Capitalize`, `Repeat`                 |
| `math`    | `AbsoluteValue`, `SquareRoot`, `Floor`, `Ceil`, `Round`, `Minimum`, `Maximum`, `Clamp`, `Sum`, `Average`, `RandomInterval` |
| `types`   | `Convert`, `IsConvertible`                                                                                                 |

`math:RandomInterval(minimum, maximum)` возвращает `int` с **обеими включёнными границами**; функция не предназначена для криптографии. Для `types:Convert(value, targetType)` и `types:IsConvertible(value, targetType)` требуется литерал типа `int`, `flt`, `str` или `bool`. Описание аргументов и пограничных случаев — в [Wiki](https://github.com/ItsMeQuote/epholang/wiki).

## Ошибки, указывающие на проблему

<img src="images/epholang_hexer_error.png" align="right" align="right" alt="Хексер с ошибкой">

В Epholang три этапа проверки ошибок:

| Ошибка          | Когда                | Сообщается              |
| --------------- | -------------------- | ----------------------- |
| `Syntax error`  | При чтении исходника | Первая ошибка           |
| `Static error`  | До запуска программы | Все обнаруженные ошибки |
| `Runtime error` | Во время выполнения  | Первая ошибка           |

Диагностика показывает файл, строку, столбец и фрагмент кода. Статическая проверка обнаруживает несовместимость типов, неиспользуемые переменные и отсутствие `return`.

## Переход с 0.1.0

В **0.2.0 есть несовместимые изменения**: тип `num` разделён на `int` и `flt`; функции объявляются как `function Name(...) type { ... }`; параметры по умолчанию удалены; встроенные функции пишутся с заглавной буквы (`Output` вместо `output`); ключевое слово `cycle` заменено на **`loop`**.

Полная история изменений — в [CHANGELOG.ru.md](CHANGELOG.ru.md). Развёрнутые описания и примеры размещаются в [Wiki](https://github.com/ItsMeQuote/epholang/wiki).

## Текущие ограничения

Union-типы, вложенные списки, пользовательские модули, обработка ошибок как значений, пользовательские типы и нативная компиляция не реализованы. Epholang пока не предназначен для замены промышленных языков программирования.

## О проекте

Реализация Epholang написана **с помощью ИИ**. Сам язык — его синтаксис, семантика, правила и проектные решения — разработаны [ItsMeQuote](https://github.com/ItsMeQuote).

Новости и обсуждение (на русском): [t.me/epholang](https://t.me/epholang).

Лицензия — [MIT](LICENSE).
