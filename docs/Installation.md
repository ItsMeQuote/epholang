# Install Epholang 0.3 Beta

Download the CLI archive and the optional VS Code extension from [GitHub Releases](https://github.com/ItsMeQuote/epholang/releases/tag/0.3.0).

| System | Architecture | Filename ending |
| --- | --- | --- |
| Windows | Intel/AMD x86-64 | `x86_64-pc-windows-msvc.zip` |
| Windows | ARM64 | `aarch64-pc-windows-msvc.zip` |
| macOS | Intel | `x86_64-apple-darwin.tar.gz` |
| macOS | Apple Silicon | `aarch64-apple-darwin.tar.gz` |
| Linux with glibc | x86-64 or ARM64 | `x86_64-unknown-linux-gnu.tar.gz` or `aarch64-unknown-linux-gnu.tar.gz` |
| Alpine or Linux with static musl | x86-64 or ARM64 | `x86_64-unknown-linux-musl.tar.gz` or `aarch64-unknown-linux-musl.tar.gz` |
| VS Code | Same package on all platforms | `epholang-language-0.3.0.vsix` |

On Linux or macOS, `uname -m` reports the machine architecture. `arm64` and `aarch64` both refer to 64-bit ARM here. On Windows, consult **Settings → System → About → System type**. Linux glibc builds use Ubuntu 22.04 as the build baseline and were smoke-tested with Debian 12, Ubuntu 22.04 and Fedora. The musl builds were smoke-tested with Alpine. See the [RC run](https://github.com/ItsMeQuote/epholang/actions/runs/45407953026) for the exact runner matrix.

## Verify and extract

Download `SHA256SUMS` alongside the archive. Compare the archive's digest with the row for its exact filename.

- **Linux:** run `sha256sum -c SHA256SUMS --ignore-missing` in the download directory.
- **macOS:** run `shasum -a 256 epholang-0.3.0-aarch64-apple-darwin.tar.gz`, using the actual downloaded filename.
- **Windows PowerShell:** run `Get-FileHash .\epholang-0.3.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256`, using the actual downloaded filename.

On Windows use **Extract All**. The archive contains `epholang.exe`. Invoke it from the extracted directory with `.\epholang.exe run hello.epho`, or add that directory to your user PATH. On Linux and macOS run `tar -xzf <archive.tar.gz>` with the downloaded archive path, then use `./epholang run hello.epho`. To invoke `epholang` by name, put the executable in a directory already on PATH.

## Build from source

Install Git, Cargo, and Rust 1.85 or later. Build the released source by selecting its tag.

```sh
git clone https://github.com/ItsMeQuote/epholang.git
cd epholang
git checkout 0.3.0
cargo build --release --locked
./target/release/epholang run examples/02_functions.epho
```

On Windows run `.\target\release\epholang.exe run examples\02_functions.epho` in PowerShell. Follow the [tutorial](Tutorial.md) to write a program. For editor setup, see [CLI and VS Code](CLI-Reference.md).
