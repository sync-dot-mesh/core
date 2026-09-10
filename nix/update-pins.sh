#!/usr/bin/env bash
# Regenerates the values needed for fully reproducible pinning.
#
# Works out of the box with NO pinning (nixpkgs-pin.nix fetches the
# nixos-24.05 branch tip impurely). Run this when you want to freeze
# to an exact commit + hash — e.g. before shipping/tagging.
#
# Requires: curl, nix-prefetch-url (ships with Nix).
set -euo pipefail

command -v nix-prefetch-url >/dev/null || {
  echo "error: nix-prefetch-url not found — install Nix first." >&2
  exit 1
}

echo "==> Resolving latest commit on nixpkgs:nixos-24.05"
sha=$(curl -sf https://api.github.com/repos/NixOS/nixpkgs/commits/nixos-24.05 \
  | grep -m1 '"sha"' | cut -d'"' -f4)

if [ -z "${sha}" ]; then
  echo "error: could not resolve nixpkgs commit (GitHub API rate limit?)." >&2
  echo "       try again shortly, or supply a commit manually:" >&2
  echo "       nix-prefetch-url --unpack https://github.com/NixOS/nixpkgs/archive/<sha>.tar.gz" >&2
  exit 1
fi

url="https://github.com/NixOS/nixpkgs/archive/${sha}.tar.gz"
echo "==> Prefetching ${url}"
hash=$(nix-prefetch-url --unpack "${url}")

cat <<EOF

Paste into nix/nixpkgs-pin.nix, in the "pinned" block:

  rev    = "${sha}";
  sha256 = "${hash}";

Then set 'source = pinned;' at the bottom of that file.

---

If nix/extensions.nix has any 'lib.fakeHash' placeholders left, run:

  nix-shell --run true

It will fail on the first such extension and print the real hash in
the error message. Paste it in, save, re-run. Repeat until it
succeeds — each run resolves exactly one placeholder.
EOF
