import { readFileSync } from "node:fs";
import snapshot from "./outputs.json";
import release from "./release.json";
export const outputs: Record<string, string> = snapshot;
export const code = (file: string) =>
  readFileSync(
    new URL("../../../examples/" + file, import.meta.url),
    "utf8",
  ).trimEnd();
export const buildCode =
  "git clone https://github.com/ItsMeQuote/epholang.git\ncd epholang\ngit checkout " +
  release.tag +
  "\ncargo build --release --locked";
