# Contributing to Epholang

Thank you for your interest in contributing to Epholang!

Epholang is an experimental programming language focused on **explicitness, readability, and strict static checking**.

Contributions are welcome, including bug fixes, tests, documentation, examples, development tools, and proposals for new language features.

## Project philosophy

Epholang follows several deliberate principles:

- **One way to do a thing.** Avoid unnecessary overlap between features.
- **Explicit over short.** Prefer clarity over syntactic shortcuts.
- **Strict, not forgiving.** Invalid programs should produce clear errors.
- **Consistency.** Features should integrate with existing language rules.
- **Simplicity.** Avoid complexity without a clear benefit.

Some decisions, including 1-based indexing and the absence of classical OOP, are intentional.

The language's creator, [ItsMeQuote](https://github.com/ItsMeQuote), has final authority over language design and contribution acceptance.

## Before contributing

Please review the following resources:

- [README](README.md) — project overview.
- [Roadmap](ROADMAP.md) — development plans.
- [Changelog](CHANGELOG.md) — release history.
- [GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki) — language documentation.

Check existing Issues and Pull Requests before starting work.

### When to open an Issue

**Open an Issue before implementing:**

- Changes to language syntax or semantics.
- Changes to the type system.
- New language features.
- Significant architectural changes.

For small bug fixes, tests, and minor documentation corrections, an Issue is not required.

For larger documentation changes, opening an Issue first is recommended.

## Development setup

Epholang is implemented in Rust.

Requirements:

- Rust 1.85 or newer.
- Cargo and Git.
- Node.js and npm for VS Code extension development.

Clone the repository:

```sh
git clone https://github.com/ItsMeQuote/epholang.git
cd epholang
```

Build the interpreter:

```sh
cargo build
```

Run an example:

```sh
cargo run -- run examples/01_hello.epho
```

## Code style

Follow standard Rust conventions.

- Use `rustfmt` for formatting.
- Address Clippy warnings.
- Prefer readable, maintainable implementations.
- Use meaningful names.
- Avoid unrelated changes in focused Pull Requests.

For Epholang examples, follow the existing naming conventions:

- **PascalCase** for function names.
- **camelCase** for variables and module aliases.

These naming conventions are not enforced by the interpreter.

## Testing and CI

Pull Requests must pass the applicable CI checks before being merged.

For Rust changes, run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo check --all-targets --locked
cargo test --all-targets --locked
```

For changes to the VS Code extension:

```sh
cd editors/vscode
npm ci
npm test
```

Bug fixes and changes to interpreter behavior should include appropriate tests whenever technically feasible.

Documentation-only or visual changes generally do not require new automated tests.

Contributors should explain how their changes were tested.

## Language design proposals

New language features, built-in functions, standard modules, and syntax changes may be proposed.

A proposal should explain:

- What problem it solves.
- Why existing language features are insufficient.
- How the proposed feature works.
- How it aligns with Epholang's design principles.
- Potential limitations or disadvantages.

The project maintainer evaluates and makes the final decision on proposals.

### Breaking changes

Epholang is currently experimental.

Breaking changes are permitted before version 1.0, but should have a clear justification and appropriate documentation.

Backward compatibility is not guaranteed during the 0.x stage.

## Documentation and translations

Documentation improvements are welcome.

For substantial changes, opening an Issue beforehand is recommended.

**When modifying documentation that exists in multiple languages, update all corresponding language versions in the same contribution.**

For example, changes to `README.md` should also be reflected in `README.ru.md`.

Documentation must describe actual implemented behavior, not planned or hypothetical features.

## AI-assisted contributions

AI-assisted contributions are allowed.

**Contributors must disclose the use of AI tools** in their Pull Request descriptions.

Contributors remain responsible for:

- Understanding submitted changes.
- Reviewing generated code.
- Testing the implementation.
- Correcting errors.
- Ensuring compliance with project requirements.

AI-generated output is not exempt from code review or testing.

## Pull Requests

1. Fork the repository and create a branch.
2. Make focused changes.
3. Add or update tests where appropriate.
4. Update relevant documentation and translations.
5. Run applicable checks.
6. Open a Pull Request describing the changes.

The PR description should include:

- A summary of changes.
- The reason for the changes.
- Testing performed.
- Related Issues, if applicable.
- Disclosure of AI assistance, if used.

### Review and acceptance

All contributions are reviewed according to correctness, maintainability, and consistency with Epholang's goals.

Contributors may be asked to revise their changes.

Pull Requests may be closed without merging if they conflict with the project's design, introduce unnecessary complexity, or leave significant review concerns unresolved.

The maintainer has final authority over acceptance.

## Communication

English and Russian are the recommended languages for Issues, Pull Requests, and discussions.

Other languages are not prohibited.

Please communicate respectfully and keep technical discussions constructive.

A separate Code of Conduct will describe community standards in more detail.

## License

By submitting a contribution, you agree that it will be distributed under the project's [MIT License](LICENSE).

## Thank you

Thank you for helping develop Epholang!
