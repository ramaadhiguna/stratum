# Stratum
A Rust-powered data infrastructure layer for managing data warehouses, data models, and unified data access through a single API.

## Prerequisites

These steps are for Linux (Debian/Ubuntu), including WSL2 on Windows. For other platforms, see [rustup.rs](https://rustup.rs).

### 1. System build tools

Rust uses the system linker to produce the final binary. Some common crates also link against OpenSSL.

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev
```

### 2. Rust toolchain (via rustup)

`rustup` installs and manages Rust toolchains. The default installation includes `rustc`, `cargo`, `clippy` and `rustfmt`.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # choose "1) Proceed with standard installation"
source "$HOME/.cargo/env"
```

This project uses the 2024 edition, so it needs **Rust 1.85 or newer**. Run `rustup update` to get the latest stable release.

### 3. Verify the installation

```bash
rustup --version
rustc --version
cargo --version
cargo clippy --version
cargo fmt --version
```

### 4. Editor setup (VS Code)

If you use WSL, install these extensions **in WSL** rather than locally on Windows:

- **rust-analyzer** (`rust-lang.rust-analyzer`): language server, required
- **Even Better TOML** (`tamasfe.even-better-toml`): `Cargo.toml` support
- **CodeLLDB** (`vadimcn.vscode-lldb`): debugger, optional

Optionally, set `rust-analyzer.check.command` to `clippy` to show lint warnings inline.
