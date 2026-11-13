import { readFileSync, existsSync } from "node:fs";
import assert from "node:assert/strict";
const r = JSON.parse(readFileSync("src/data/release.json", "utf8"));
assert(
  r.tag &&
    r.name &&
    r.url === `https://github.com/ItsMeQuote/epholang/releases/tag/${r.tag}`,
);
assert(r.prerelease === true || r.prerelease === false);
for (const a of r.assets) {
  assert(["interpreter", "editor"].includes(a.kind));
  assert(
    a.url.startsWith(
      `https://github.com/ItsMeQuote/epholang/releases/download/${r.tag}/`,
    ),
  );
  assert(
    decodeURIComponent(new URL(a.url).pathname.split("/").at(-1)) === a.name,
  );
}
for (const file of [
  "01_hello",
  "02_functions",
  "03_loops",
  "04_lists",
  "05_strings",
  "06_math",
  "07_types",
])
  assert(existsSync(`../examples/${file}.epho`));
assert(existsSync("../editors/vscode/syntaxes/epholang.tmLanguage.json"));
console.log("Release metadata, example sources and grammar validated.");
