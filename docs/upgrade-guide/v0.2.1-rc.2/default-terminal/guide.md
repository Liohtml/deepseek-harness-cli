---
kind: upgrade-guide
description: "Bare dsh now starts an interactive coding session instead of rejecting a missing profile."
---

# `dsh` starts the terminal agent by default

English | [中文](guide.zh.md)

## Change

In this fork, `dsh` without arguments starts `headless --interactive`. Previously it exited with a missing-profile error. Explicit profile commands keep their meaning. The separately installed [Rust frontend](../../../../native/cli/README.md) opens the same conversation through `headless --terminal-bridge`; its `--version` reports the frontend version.

## Migration

1. Use `dsh` in an interactive terminal. Automation must continue to select a profile explicitly, for example `dsh headless "run the tests"`; no-argument piped invocations fail with terminal guidance.
2. Use `dsh --help` for launcher help. With the Rust frontend, use `dsh headless --version` to query the runtime version.
3. Start `dsh` inside a project and enter `/exit` to verify startup and clean shutdown without a model request.

4. The Rust frontend now uses a full-screen Ratatui interface and terminal protocol version 2. Rebuild the Node runtime and reinstall the Rust executable together; a version 1 executable cannot connect to the new bridge. Rust 1.88 or newer is required. Use `dsh --demo` to preview the interface without an API key. For source launches, generate Typert artifacts with `node --import tsx scripts/generate-typert.ts` before starting.
