#!/usr/bin/env bash
# One command, run once after cloning (and safe to re-run any time):
#
#   ./init_devenv.sh
#
# - Confirms direnv is installed and the shell hook is set up.
# - direnv allow  — trusts .envrc (required before direnv loads
#                    anything; also re-confirms trust if .envrc's
#                    content has changed since it was last allowed).
# - direnv reload — forces immediate re-evaluation right now, even if
#                    .envrc was already allowed and unchanged. Without
#                    this, an already-trusted, unchanged .envrc is
#                    served from cache and nothing visibly happens
#                    until the next `cd` into the directory.
#
# After this, the environment (rustc, cargo, clippy, rustfmt, the
# isolated vscodium/codium, ...) loads automatically every time you
# `cd` into this directory. You will not need to run this script
# again unless you want to force an immediate reload without waiting
# for the next `cd` — e.g. right after editing shell.nix.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

if ! command -v direnv >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: direnv is not installed.

Install it, then re-run this script:
  nix-env -iA nixpkgs.direnv     # if you have Nix but no direnv yet
  brew install direnv            # macOS
  sudo apt install direnv        # Debian/Ubuntu
  sudo pacman -S direnv          # Arch

Then add the shell hook to your shell's rc file — a one-time, global,
per-machine step direnv needs to work at all:
  bash: echo 'eval "$(direnv hook bash)"' >> ~/.bashrc
  zsh:  echo 'eval "$(direnv hook zsh)"'  >> ~/.zshrc
  fish: echo 'direnv hook fish | source'  >> ~/.config/fish/config.fish

Open a new terminal afterwards, then re-run ./init_devenv.sh.
EOF
  exit 1
fi

if [ ! -f .envrc ]; then
  echo "use nix" > .envrc
  echo "==> .envrc was missing, recreated it"
fi

echo "==> direnv allow"
direnv allow

echo "==> direnv reload (forces evaluation now, not on next cd)"
direnv reload

cat <<'EOF'

Done. rustc, cargo, clippy, rustfmt, and the isolated vscodium/codium
are loaded for this directory via direnv.

Note: this script ran as a subprocess, which cannot export new
environment variables into the shell that launched it — that's a
shell limitation, not a bug here. If `cargo --version` or `vscodium`
doesn't resolve in THIS exact terminal, run `cd .` once (or open a
new terminal) and direnv's hook will pick up the already-cached
environment instantly.
EOF
