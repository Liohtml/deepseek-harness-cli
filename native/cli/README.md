# Native DSH terminal

English | [中文](README.zh.md)

The Rust executable `dsh` opens a coding session in the current project without arguments. It owns terminal editing, in-memory history, bracketed paste, rendering, and interrupt handling. The local Node.js agent runs through the existing `headless` profile over a versioned JSON-lines transport; no web server is started. This is a native frontend, not a Rust rewrite of the agent engine.

## Installation

Install Rust with Cargo and Node.js `^22.19` or `>=24`, then build this fork and install its executable:

```sh
pnpm install
pnpm run build
./install.sh
```

The installer defaults to `~/.local/bin/dsh`; set `DSH_INSTALL_PREFIX` to choose another prefix. Add that prefix's `bin` directory to `PATH` if the installer reports it missing. An unrelated existing `dsh` binary is not overwritten by Cargo. Use `command -v dsh` to check which installation your shell selects.

Keep the built checkout at its installation path: the binary records its `apps/cli/lib/bin.js` location. Moving the checkout requires reinstalling or setting `DSH_RUNTIME_JS` to the relocated entry. Node and the checkout's dependencies are required at runtime. `DSH_NODE` selects a Node executable. The upstream npm release does not contain this fork. The shell installer targets Linux/macOS; on Windows, build the runtime and use `cargo install --locked --path native/cli`.

## Usage

```sh
cd /path/to/project
export DEEPSEEK_API_KEY=your-key
dsh
dsh --resume session-…
dsh --cwd /path/to/another-project
dsh headless "run the tests"
dsh web
```

The project `.env` can supply the API key instead. `/help`, `/session`, `/multiline`, `/exit`, approvals, and persistence follow the [terminal reference](../../packages/bundle/headless/README.md#interactive-terminal). Arrow keys edit input and browse up to 100 prompts in memory; approval answers never enter history. Bracketed paste preserves multiline text and waits for Enter. Ctrl+C cancels a running turn or exits at the prompt; Ctrl+D closes the session. Piped tasks use the explicit `headless` command. Explicit profiles are forwarded verbatim to the Node launcher. `dsh --version` reports the frontend version; `dsh headless --version` reports the agent runtime version.

## Development

```sh
cargo test --locked --manifest-path native/cli/Cargo.toml
cargo clippy --locked --manifest-path native/cli/Cargo.toml --all-targets -- -D warnings
cargo build --locked --manifest-path native/cli/Cargo.toml
python3 native/cli/tests/terminal_smoke.py
```

The POSIX PTY tests use a fake runtime without API requests. For source development, set `DSH_RUNTIME_JS` to the absolute `apps/cli/src/bin.ts` path and `NODE_OPTIONS='--import tsx/esm'`, with source dependencies installed.
