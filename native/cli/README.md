# Native DSH terminal

English | [中文](README.zh.md)

The Rust executable `dsh` opens a coding session in the current project without arguments. Its Ratatui interface provides a fixed multiline composer, a scrollable conversation, streaming answers, code blocks, tool results, and distinct permission prompts. It owns terminal editing, in-memory history, bracketed paste, and interrupt handling. The local Node.js agent runs through the existing `headless` profile over a versioned JSON-lines transport; no web server is started. This is a native frontend, not a Rust rewrite of the agent engine.

## Installation

Install Rust 1.88 or newer with Cargo and Node.js `^22.19` or `>=24`, then build this fork and install its executable:

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
dsh
dsh --resume session-…
dsh --cwd /path/to/another-project
dsh headless "run the tests"
dsh web
dsh --demo
```

The project `.env` can supply the API key instead. `/help`, `/session`, `/multiline`, `/exit`, approvals, and persistence follow the [terminal reference](../../packages/bundle/headless/README.md#interactive-terminal). Arrow keys edit input and browse up to 100 prompts in memory; approval answers never enter history. Bracketed paste preserves multiline text and waits for Enter. Ctrl+C cancels a running turn or exits at the prompt; Ctrl+D closes the session. Piped tasks use the explicit `headless` command. Explicit profiles are forwarded verbatim to the Node launcher. `dsh --version` reports the frontend version; `dsh headless --version` reports the agent runtime version.

`dsh --demo` previews the interface without Node, credentials, or model requests. In a terminal, press 1 for the welcome screen, 2 for a sample conversation, or 3 for an approval; Esc exits. With redirected output it prints a static preview. Example tool results in this mode are fixtures.

| Key | Action |
|---|---|
| Enter | Send the message or answer |
| Alt+Enter / Ctrl+J | Insert a newline |
| PgUp / PgDn | Scroll conversation or diagnostics |
| Ctrl+End | Return to the latest message |
| Ctrl+T | Expand tool arguments and results |
| F1 | Open keyboard help |
| F2 / `/diagnostics` | Inspect runtime warnings and errors |
| `/` or Ctrl+P, then ↑ / ↓ and Enter | Select and run a command; Tab completes it |
| `/clear` | Clear the display while preserving agent context |

The UI uses the alternate screen and restores the terminal on exit. Runtime diagnostics remain accessible through F2; startup failures print their diagnostics after restoring the shell. Rendering requires at least 28 columns and 10 rows; 80 × 24 or larger is recommended.

<a id="connect-and-configure"></a>

## Connect and configure

Run `/login` inside `dsh` and paste your DeepSeek API key into the masked field. Enter saves it; Esc cancels. The key is excluded from screen output, prompt history, Agent messages, and Session logs. It is stored through the existing [credential provider](../../packages/credentials/credentials-local/README.md) in `$DSH_HOME/.credentials.yaml` (POSIX mode `0600`), not an encrypted OS keychain; processes running as your user can read it. This is API-key authentication, not a browser account sign-in.

| Command | Action |
|---|---|
| `/login` | Save or replace the DeepSeek API key through masked input |
| `/logout` | Confirm removal of the saved key; report environment or `.env` fallbacks |
| `/status` | Show workspace, selected model, credential presence and source |
| `/model` | Choose an advertised model for subsequent prompts in this session |
| `/check` | Send a short connection-check prompt through the selected model; normal API charges apply |
| `/sessions` | List up to ten saved sessions in this project with resume commands |

Never pass keys as slash-command arguments. `/login` manages `DEEPSEEK_API_KEY`; custom provider credential references remain configured through their existing environment or settings. A saved key is not validated until an API request succeeds. Model selection lasts for the current process; future launches use the configured default.

<a id="development"></a>

## Development

```sh
cargo test --locked --manifest-path native/cli/Cargo.toml
cargo clippy --locked --manifest-path native/cli/Cargo.toml --all-targets -- -D warnings
cargo build --locked --manifest-path native/cli/Cargo.toml
python3 -m pip install -r native/cli/tests/requirements.txt
python3 native/cli/tests/terminal_smoke.py
```

The POSIX PTY tests use a fake runtime without API requests and inspect the rendered terminal with Pyte. For source development, first build the host system addon, generate the Typert artifacts, and rebuild the native executable from the same checkout:

```sh
node --import tsx native/system/scripts/build.ts --host-addon-only
node --import tsx scripts/generate-typert.ts
cargo build --release --locked --manifest-path native/cli/Cargo.toml
```

Then set `DSH_RUNTIME_JS` to the absolute `apps/cli/src/bin.ts` path and `NODE_OPTIONS='--import tsx/esm'`, with source dependencies installed.

The frontend and backend use terminal protocol version 3. After updating either side, rebuild both together. Missing `lib/typert.host.js` errors mean the runtime artifacts have not been generated; run the generation command above for source development or `pnpm run build` for an installed runtime.

Missing `system.node` means the host system addon has not been built; run the host-addon command above. It supplies the file lock required to save and resume Sessions. A backend failure restores the terminal and prints its diagnostics before exiting with a nonzero status.
