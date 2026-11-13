import { readFileSync, writeFileSync } from "node:fs";
const args = process.argv.slice(2),
  index = args.indexOf("--tag");
if (index < 0 || !args[index + 1])
  throw Error("Usage: npm run release:update -- --tag 0.2.0 [--check]");
const tag = args[index + 1];
const response = await fetch(
  `https://api.github.com/repos/ItsMeQuote/epholang/releases/tags/${encodeURIComponent(tag)}`,
  { headers: { Accept: "application/vnd.github+json" } },
);
if (!response.ok)
  throw Error(`GitHub returned ${response.status}; snapshot unchanged`);
const release = await response.json();
if (release.draft) throw Error("Draft releases are not publishable");
const current = JSON.parse(readFileSync("src/data/release.json", "utf8"));
const assets = release.assets.map((a) => {
  const previous = current.assets.find((p) => p.name === a.name);
  let kind = previous?.kind,
    platform = previous?.platform;
  if (a.name.endsWith(".vsix")) {
    kind = "editor";
    platform = "VS Code";
  }
  if (!kind || !platform)
    throw Error(
      `Review new asset ${a.name} and add its explicit kind/platform mapping to release.json, then rerun`,
    );
  return {
    id: a.id,
    name: a.name,
    url: a.browser_download_url,
    platform,
    kind,
    size: a.size,
    digest: a.digest ?? null,
  };
});
if (
  !assets.some((a) => a.kind === "interpreter") ||
  !assets.some((a) => a.kind === "editor")
)
  throw Error(
    "Expected interpreter and editor assets; review release before updating",
  );
const next = {
  tag: release.tag_name,
  name: release.name || release.tag_name,
  prerelease: release.prerelease,
  publishedAt: release.published_at,
  checkedAt: new Date().toISOString().slice(0, 10),
  url: release.html_url,
  assets,
};
if (args.includes("--check")) {
  for (const a of current.assets) {
    if (!assets.some((b) => a.name === b.name && a.url === b.url))
      throw Error(`Missing or changed asset: ${a.name}`);
  }
  console.log(
    `Verified ${tag}: ${assets.length} published assets; no files changed.`,
  );
} else {
  writeFileSync("src/data/release.json", JSON.stringify(next, null, 2) + "\n");
  console.log(
    `Updated snapshot for ${tag}; review the diff before committing.`,
  );
}
