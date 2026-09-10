/*
  Pinned nixpkgs — classic (non-flake) style.

  Default: tracks the nixos-24.05 branch tarball with NO sha256. Works
  immediately with plain `nix-shell` / direnv — Nix fetches it
  impurely on first eval (network required once, then cached in the
  local Nix store like anything else).

  For a fully reproducible, byte-for-byte pin, run:

      ./nix/update-pins.sh

  and paste the printed `rev` + `sha256` into the commented block
  below, then swap `source = unpinned;` to `source = pinned;`.
*/
{ }:

let
  unpinned = builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/refs/heads/nixos-24.05.tar.gz";
  };

  # pinned = builtins.fetchTarball {
  #   url    = "https://github.com/NixOS/nixpkgs/archive/<REV>.tar.gz";
  #   sha256 = "<SHA256>";
  # };

  source = unpinned;
in
import source { }
