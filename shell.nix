/*
  The dev shell. Consumed by `nix-shell` directly, or ambiently via
  direnv (see .envrc / init_devenv.sh) — you should rarely need to
  invoke nix-shell by hand once direnv is set up.
*/
let
  pkgs = import ./nix/nixpkgs-pin.nix { };

  vscodiumRust = import ./nix/vscodium.nix { inherit pkgs; };

  rustToolchain = with pkgs; [
    rustc
    cargo
    rust-analyzer
    clippy
    rustfmt

    cargo-edit
    cargo-watch
    cargo-nextest
    cargo-audit
    cargo-outdated

    pkg-config
    openssl
    lldb
  ]
  ++ lib.optionals stdenv.isLinux [ mold ];

  # The .nix files ARE the backbone of this boilerplate, not an
  # afterthought — full language server, formatter, and linters, same
  # tier of support as the Rust toolchain above.
  nixToolchain = with pkgs; [
    nixd        # LSP — eval-based completion, goes-to-definition into nixpkgs itself
    alejandra   # formatter
    statix      # linter — anti-patterns, unnecessary constructs
    deadnix     # finds unused bindings
  ];

  inherit (pkgs) lib stdenv;
in
pkgs.mkShell {
  buildInputs = rustToolchain ++ nixToolchain ++ [ vscodiumRust ];

  shellHook = ''
    echo ""
    echo "  Rust + Nix dev shell ready."
    echo "  Isolated IDE, project-local — never touches your global VSCodium."
    echo ""
    echo "    vscodium .   open the isolated editor here (also: codium)"
    echo "    cargo ...    rustc/cargo/clippy/rustfmt are all on PATH"
    echo "    nixd, alejandra, statix, deadnix are all on PATH too —"
    echo "    the .nix files get the same tier of IDE support as the Rust code."
    echo ""
  '';
}
