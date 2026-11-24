import { ru } from "./site.ru";
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
    ru.home,
    ru.getStartedText,
    ru.documentation,
    ru.examples,
    ru.download,
    ru.about,
  ],
  en: ["Home", "Get started", "Documentation", "Examples", "Download", "About"],
};
export const url = (lang: Lang, slug = "") =>
  `${base}${lang}/${slug ? slug + "/" : ""}`;
export const t = (lang: Lang, ru: string, en: string) =>
  lang === "ru" ? ru : en;
export const descriptions = {
  ru: [
    ru.epholang,
    ru.epholangGithubWiki,
    ru.epholangText,
    ru.epholangWindowsVsCode,
    ru.epholangTextText,
    ru.anExperimentalProgrammingLanguageWithExplicitTypesAnd,
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
    ru.explicitTypesText,
    "Explicit types",
    ru.intAndFltAreDistinctConversionsAreExplicit,
    "int and flt are distinct. Conversions are explicit.",
    "int · flt · str · bool",
  ],
  [
    ru.checkBeforeRunning,
    "Check before running",
    ru.typeErrorsUnusedNamesRecursionAndMissingReturns,
    "Type errors, unused names, recursion and missing returns prevent execution.",
    "epholang check",
  ],
  [
    ru.typedFunctions,
    "Typed functions",
    ru.functionHeadersDeclareAResultTypeWithOr,
    "Function headers declare a result type with -> or mark the absence of a value with -|.",
    "function Square(int value) -> int:",
  ],
  [
    ru.listsStartAtOne,
    "Lists start at one",
    ru.aListCanDeclareAUnionElementType,
    "A list can declare a union element type. Its first index is 1.",
    "numbers[1]",
  ],
  [
    ru.expressiveStrings,
    "Expressive strings",
    ru.embedExpressionsWithNewlinesAreExplicit,
    "Embed expressions with #{...}. Newlines are explicit.",
    'Output("Hello, #{name}!\\n")',
  ],
  [
    ru.threeStandardModules,
    "Three standard modules",
    ru.stringsMathematicsAndTypeConversionsThroughBuiltIn,
    "Strings, mathematics and type conversions through built-in modules.",
    "strings · math · types",
  ],
];
export const limits = {
  ru: ru.theLanguageIsExperimentalNestedListsCustomTypes,
  en: "The language is experimental. Nested lists, custom types and modules, errors as values and native compilation are unavailable. Breaking changes are possible.",
};
export const examples = [
  {
    file: "01_hello.epho",
    ru: ru.aFirstHello,
    en: "A first hello",
    r: ru.theBuiltInOutputPrintsAStringUse,
    e: "The built-in Output prints a string. Use \n for a newline.",
  },
  {
    file: "02_functions.epho",
    ru: ru.aFunctionAndItsResult,
    en: "A function and its result",
    r: ru.squareReturnsANumberDisplaysquarePrintsItAnd,
    e: "Square returns a number. DisplaySquare prints it and finishes.",
  },
  {
    file: "03_loops.epho",
    ru: ru.twoKindsOfLoop,
    en: "Two kinds of loop",
    r: ru.aConditionalLoopAndIterationOverAList,
    e: "A conditional loop and iteration over a list.",
  },
  {
    file: "04_lists.epho",
    ru: ru.workingWithLists,
    en: "Working with lists",
    r: ru.insertReturnsANewListIndexingStartsAt,
    e: "Insert returns a new list. Indexing starts at one.",
  },
  {
    file: "05_strings.epho",
    ru: ru.theStringsModule,
    en: "The strings module",
    r: ru.moduleCallsUseAColonStringsSplit,
    e: "Module calls use a colon: strings:Split(...).",
  },
  {
    file: "06_math.epho",
    ru: ru.mathematics,
    en: "Mathematics",
    r: ru.sumAverageAndARandomIntegerWithInclusive,
    e: "Sum, average and a random integer with inclusive bounds.",
  },
  {
    file: "07_types.epho",
    ru: ru.explicitConversion,
    en: "Explicit conversion",
    r: ru.checkConvertibilityBeforeCallingConvert,
    e: "Check convertibility before calling Convert.",
  },
];
