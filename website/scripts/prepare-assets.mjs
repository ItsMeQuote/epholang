import { copyFileSync, mkdirSync } from "node:fs";
mkdirSync("public/assets", { recursive: true });
mkdirSync("public/fonts", { recursive: true });
for (const name of [
  "epholang_logo_icon.png",
  "epholang_logo.png",
  "epholang_hexer_laptop.png",
])
  copyFileSync("../images/" + name, "public/assets/" + name);
for (const [family, short] of [
  ["inter", "inter"],
  ["jetbrains-mono", "mono"],
]) {
  for (const subset of ["latin", "cyrillic"])
    copyFileSync(
      `node_modules/@fontsource/${family}/files/${family}-${subset}-400-normal.woff2`,
      `public/fonts/${short}-${subset}.woff2`,
    );
  copyFileSync(
    `node_modules/@fontsource/${family}/LICENSE`,
    `public/fonts/${family}-LICENSE.txt`,
  );
}
for (const subset of ["latin", "cyrillic"])
  copyFileSync(
    `node_modules/@fontsource/inter/files/inter-${subset}-600-normal.woff2`,
    `public/fonts/inter-${subset}-600.woff2`,
  );
