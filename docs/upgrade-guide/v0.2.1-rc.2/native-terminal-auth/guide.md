---
kind: upgrade-guide
description: "Native terminal protocol 3 requires rebuilding the Rust frontend and Node runtime together for masked credential input."
---

# Native terminal authentication

English | [中文](guide.zh.md)

## Change

The native terminal transport uses protocol 3 for credential status and masked input. Protocol 2 frontends and backends cannot be mixed with protocol 3. A mismatched pair refuses the handshake before accepting credentials. Existing Session logs and stored credentials are unchanged.

Interactive headless sessions now read the managed credential store before environment fallbacks. One-shot jobs keep their environment-based credential selection.

## Migration

1. In the updated checkout, run `pnpm install`, `pnpm run build`, and `./install.sh` to rebuild both components. For source development, follow the [native build instructions](../../../../native/cli/README.md#development).
2. Restart `dsh`. Run `/login` without arguments and paste the API key into the masked field; do not pass it in the command line.
3. Confirm `/status` reports the key as configured. `/check` makes a short model request to verify access; normal API charges apply. Storage protection and removal are described in the [terminal reference](../../../../native/cli/README.md#connect-and-configure).
4. To use an environment key instead of a saved DeepSeek key, run `/logout` and confirm removal. `/status` reports the remaining credential source.
