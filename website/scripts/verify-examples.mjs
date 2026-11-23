import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
const files = [
  "01_hello",
  "02_functions",
  "03_loops",
  "04_lists",
  "05_strings",
  "06_math",
  "07_types",
];
const outputs = {};
for (const file of files) {
  const r = spawnSync(
    process.env.EPHOLANG_BIN || "../target/debug/epholang",
    ["run", `../examples/${file}.epho`],
    { encoding: "utf8", timeout: 10000 },
  );
  if (r.error || r.status !== 0) throw Error(`${file}: ${r.error || r.stderr}`);
  if (file !== "06_math") outputs[file + ".epho"] = r.stdout;
}
const path = "src/data/outputs.json";
if (process.argv.includes("--write"))
  writeFileSync(path, JSON.stringify(outputs, null, 2) + "\n");
else if (
  JSON.stringify(JSON.parse(readFileSync(path, "utf8"))) !==
  JSON.stringify(outputs)
)
  throw Error(
    "Verified output snapshot differs; run verify-examples.mjs --write and review",
  );
console.log(
  "Seven programs executed successfully; six deterministic output snapshots verified.",
);
