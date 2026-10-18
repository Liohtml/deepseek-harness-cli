#!/usr/bin/env bash
# Install the native frontend; the built checkout remains its runtime installation.
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
install_prefix="${DSH_INSTALL_PREFIX:-${HOME}/.local}"
if [[ $# -ne 0 ]]; then
  printf 'Usage: DSH_INSTALL_PREFIX=/path/to/prefix ./install.sh\n' >&2
  exit 2
fi
for executable in node cargo; do
  if ! command -v "$executable" >/dev/null 2>&1; then
    printf 'Missing %s. Install Node.js and Rust before installing DSH.\n' "$executable" >&2
    exit 1
  fi
done
node -e 'const [major, minor] = process.versions.node.split(".").map(Number); if (!(major === 22 && minor >= 19 || major >= 24)) { console.error("DSH needs Node.js ^22.19 or >=24"); process.exit(1) }'
if [[ ! -f "$repo_dir/apps/cli/lib/bin.js" ]]; then
  printf 'Build the runtime first in %s:\n  pnpm install\n  pnpm run build\n' "$repo_dir" >&2
  exit 1
fi
node "$repo_dir/apps/cli/lib/bin.js" --version >/dev/null
cargo install --locked --path "$repo_dir/native/cli" --root "$install_prefix"
printf '\nInstalled: %s/bin/dsh\nKeep this checkout: it contains the agent runtime.\n' "$install_prefix"
case ":${PATH}:" in
  *":${install_prefix}/bin:"*) printf 'Start in your project directory with: dsh\n' ;;
  *) printf 'Add %s/bin to PATH, reopen your terminal, then run: dsh\n' "$install_prefix" ;;
esac
