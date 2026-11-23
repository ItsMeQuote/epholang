import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import assert from "node:assert/strict";
const root = resolve("dist");
let count = 0;
const titles = new Set();
function walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      walk(p);
      continue;
    }
    if (!p.endsWith(".html")) continue;
    count++;
    const html = readFileSync(p, "utf8");
    const title = html.match(/<title>(.*?)<\/title>/s)?.[1];
    assert(title && !titles.has(title), `Duplicate/missing title: ${p}`);
    titles.add(title);
    assert(html.includes('name="description"'));
    assert(html.includes("<h1"));
    assert(
      !/\p{Extended_Pictographic}/u.test(html),
      `Unexpected emoji in ${p}`,
    );
    for (const m of html.matchAll(/(?:href|src)="([^"#]+)(?:#[^"]*)?"/g)) {
      const link = m[1];
      if (/^(https?:|data:|mailto:)/.test(link)) continue;
      let target = link.startsWith("/epholang/")
        ? resolve(root, link.slice(10))
        : resolve(dirname(p), link);
      if (link === "/" || link === "/epholang") target = root;
      assert(target.startsWith(root));
      if (existsSync(target) && statSync(target).isDirectory())
        target = join(target, "index.html");
      assert(existsSync(target), `Broken local link ${link} in ${p}`);
    }
  }
}
walk(root);
assert(count === 14, `Expected 14 pages, got ${count}`);
const rootPage = readFileSync("dist/index.html", "utf8");
assert(
  rootPage.includes('href="/epholang/ru/"') &&
    rootPage.includes('href="/epholang/en/"'),
);
console.log(
  `${count} HTML pages: unique titles, metadata, local links, assets and language choices passed.`,
);
