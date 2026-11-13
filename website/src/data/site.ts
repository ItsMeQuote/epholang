export type Lang = "ru" | "en";
export const repo = "https://github.com/ItsMeQuote/epholang";
export const base = "/epholang/";
export const slugs = [
  "",
  "getting-started",
  "docs",
  "examples",
  "download",
  "about",
] as const;
export const labels = {
  ru: [
    "Главная",
    "Начало работы",
    "Документация",
    "Примеры",
    "Загрузка",
    "О проекте",
  ],
  en: ["Home", "Get started", "Documentation", "Examples", "Download", "About"],
};
export const url = (lang: Lang, slug = "") =>
  `${base}${lang}/${slug ? slug + "/" : ""}`;
export const t = (lang: Lang, ru: string, en: string) =>
  lang === "ru" ? ru : en;
export const descriptions = {
  ru: [
    "Экспериментальный язык программирования с явными типами и строгой статической проверкой.",
    "Пошаговая установка Epholang и запуск первой программы.",
    "Краткие руководства Epholang и ссылки на GitHub Wiki.",
    "Семь настоящих примеров Epholang: функции, циклы, списки и модули.",
    "Epholang: Windows, сборка из исходников и расширение VS Code.",
    "Принципы Epholang, ограничения, авторство и участие в разработке.",
  ],
  en: [
    "An experimental programming language with explicit types and strict static checking.",
    "Install Epholang and run your first program step by step.",
    "Short Epholang guides and links to the GitHub Wiki.",
    "Seven real Epholang examples: functions, loops, lists and modules.",
    "Epholang: Windows, source builds and the VS Code extension.",
    "Epholang principles, limitations, authorship and contributing.",
  ],
};
export const features = [
  [
    "Явные типы",
    "Explicit types",
    "int и flt разделены. Преобразования выполняются явно.",
    "int and flt are distinct. Conversions are explicit.",
    "int · flt · str · bool",
  ],
  [
    "Проверка до запуска",
    "Check before running",
    "Ошибки типов, неиспользуемые имена и отсутствие return останавливают запуск.",
    "Type errors, unused names and missing returns prevent execution.",
    "epholang check",
  ],
  [
    "Типизированные функции",
    "Typed functions",
    "Типы параметров, результата и return видны в исходном коде.",
    "Parameter, result and return types are visible in the source.",
    "function Square(value int) int",
  ],
  [
    "Списки с единицы",
    "Lists start at one",
    "У каждого списка один тип элементов. Первый индекс — 1.",
    "Each list has one element type. Its first index is 1.",
    "numbers[1]",
  ],
  [
    "Выразительные строки",
    "Expressive strings",
    "Вставляйте выражения через #{...}. Перенос строки указывается явно.",
    "Embed expressions with #{...}. Newlines are explicit.",
    'Output("Hello, #{name}!\\n")',
  ],
  [
    "Три стандартных модуля",
    "Three standard modules",
    "Строки, математика и преобразования типов без пользовательских импортов.",
    "Strings, mathematics and type conversions through built-in modules.",
    "strings · math · types",
  ],
];
export const limits = {
  ru: "Язык экспериментальный. Нет вложенных списков, union-типов, пользовательских типов и модулей, обработки ошибок как значений и нативной компиляции. Возможны несовместимые изменения.",
  en: "The language is experimental. Nested lists, union types, custom types and modules, errors as values and native compilation are unavailable. Breaking changes are possible.",
};
export const examples = [
  {
    file: "01_hello.epho",
    ru: "Первое приветствие",
    en: "A first hello",
    r: "Встроенная Output выводит строку. Перенос строки задаётся через \n.",
    e: "The built-in Output prints a string. Use \n for a newline.",
  },
  {
    file: "02_functions.epho",
    ru: "Функция и результат",
    en: "A function and its result",
    r: "Явные типы параметра, результата и оператора return.",
    e: "Explicit types for the parameter, result and return statement.",
  },
  {
    file: "03_loops.epho",
    ru: "Два вида цикла",
    en: "Two kinds of loop",
    r: "Цикл по условию и последовательный обход списка.",
    e: "A conditional loop and iteration over a list.",
  },
  {
    file: "04_lists.epho",
    ru: "Работа со списками",
    en: "Working with lists",
    r: "Insert возвращает новый список. Индексация начинается с единицы.",
    e: "Insert returns a new list. Indexing starts at one.",
  },
  {
    file: "05_strings.epho",
    ru: "Строковый модуль",
    en: "The strings module",
    r: "Вызовы модуля используют двоеточие: strings:Split(...).",
    e: "Module calls use a colon: strings:Split(...).",
  },
  {
    file: "06_math.epho",
    ru: "Математика",
    en: "Mathematics",
    r: "Сумма, среднее и случайное целое с включёнными границами.",
    e: "Sum, average and a random integer with inclusive bounds.",
  },
  {
    file: "07_types.epho",
    ru: "Явное преобразование",
    en: "Explicit conversion",
    r: "Проверка возможности преобразования перед Convert.",
    e: "Check convertibility before calling Convert.",
  },
];
