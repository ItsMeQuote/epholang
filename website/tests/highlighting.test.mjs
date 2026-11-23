import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { codeToTokens } from "shiki";

const grammar = {
  ...JSON.parse(readFileSync("../editors/vscode/syntaxes/epholang.tmLanguage.json", "utf8")),
  name: "epholang",
};

async function scopes(source) {
  const { tokens } = await codeToTokens(source, {
    lang: grammar,
    theme: "github-dark",
    includeExplanation: "scopeName",
  });
  assert.equal(tokens.map(line => line.map(token => token.content).join("")).join("\n"), source);
  return tokens.map(line => line.flatMap(token => token.explanation).map(piece => ({
    text: piece.content,
    scopes: piece.scopes.map(scope => scope.scopeName),
  })));
}

function has(line, text, scope) {
  return line.some(part => part.text.includes(text) && part.scopes.includes(scope + ".epholang"));
}

test("nested quotes, interpolation and comments keep the right TextMate scopes", async () => {
  const source = String.raw`/* outer /* inner */ still comment */
Output("Result: #{strings:Repeat("Hello #{name}", 2)} #{list<str>{"}" }[1]}")`;
  const lines = await scopes(source);
  assert(has(lines[0], "still comment", "comment.block"));
  assert(has(lines[1], "Hello", "string.quoted.double"));
  assert(has(lines[1], ":", "punctuation.accessor"));
  assert(has(lines[1], "{", "punctuation.definition.list.begin"));
  assert(has(lines[1], "}", "punctuation.definition.list.end"));
  const identifier = lines[1].find(part => part.text === "name");
  assert.equal(identifier.scopes.filter(scope => scope === "meta.interpolation.epholang").length, 2);
});

test("blocks, multiline lists, union types and interpolation end in their own contexts", async () => {
  const source = [
    "function Pick(int | str value) -> str:",
    "\tif true:",
    "\t\treturn str \"#{Length(list<int>{1})} #{value}\"",
    "\telse:",
    "\t\treturn str \"none\"",
    "Output(list<",
    "\tstr",
    ">{",
    "\t\"one\",",
    "})",
    "Output(Pick(1))",
  ].join("\n");
  const lines = await scopes(source);
  assert(has(lines[0], "->", "punctuation.separator.return-type"));
  assert(has(lines[0], "|", "keyword.operator"));
  assert(has(lines[0], ":", "punctuation.section.block"));
  assert(has(lines[3], "else", "keyword.control"));
  assert(has(lines[6], "str", "meta.list.type"));
  assert(has(lines[7], "{", "punctuation.definition.list.begin"));
  assert(has(lines[8], "one", "meta.list.elements"));
  assert(has(lines[9], "}", "punctuation.definition.list.end"));
  assert(!has(lines[10], "Output", "meta.list.literal"));
  assert(has(lines[10], "Output", "support.function.builtin"));
});

test("one-line interpolation stops highlighting at a physical line break", async () => {
  const source = 'Output("broken #{/* unterminated\nOutput(1)';
  const lines = await scopes(source);
  assert(has(lines[0], "unterminated", "comment.block"));
  assert(has(lines[1], "Output", "support.function.builtin"));
  assert(!has(lines[1], "Output", "meta.interpolation"));
});
