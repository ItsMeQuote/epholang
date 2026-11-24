# Epholang website

Use Node.js 24. From this directory run `npm ci` and `npm run dev`.
Run `npm run check`, `npm run build`, `npm run check:dist` and `npm test` to validate both locales.
Build the interpreter with `cargo build --locked` in the repository root before `npm run examples:check`.
The site reads examples and the editor grammar directly from the repository. Update the download snapshot only after the release is public, with `npm run release:update -- --tag 0.3.0`.

[Russian guide](README.ru.md) · [Historical validation report (Russian)](VALIDATION.ru.md)
