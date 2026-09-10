/*
  Curated extension set for the isolated Rust IDE.

  `pkgs.vscode-extensions.*` — nixpkgs already packages and hashes
  these, zero setup. Preferred whenever available.

  `vscode-utils.extensionsFromVscodeMarketplace` — manual fallback for
  anything not in nixpkgs' curated set, pinned by version + content
  hash. New entries start with `lib.fakeHash` — the first `nix-shell`
  (or direnv load) refuses with a "hash mismatch" error that prints
  the correct hash. Paste it in. Standard Nix workflow, not a hack.
*/
{ pkgs }:

let
  inherit (pkgs) lib vscode-utils;

  marketplaceExtras = vscode-utils.extensionsFromVscodeMarketplace [
    {
      # Nix language support: syntax, LSP client wiring, formatter wiring.
      # The actual LSP (nixd) and formatter (alejandra) are Nix packages
      # on PATH — this extension is just the editor-side integration.
      #
      # Deliberately NOT the newest release. jnoortheen.nix-ide >=0.3.7
      # requires VS Code engine >=1.96.0; the VSCodium build nixpkgs'
      # nixos-24.05 branch provides is 1.94.1. 0.3.5 is the newest
      # version whose floor (>=1.91.0) that actually satisfies. If you
      # bump the nixpkgs pin to something shipping a newer VSCodium,
      # re-check compatible versions before bumping this too — query:
      #
      #   curl -s -X POST \
      #     "https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery" \
      #     -H "Content-Type: application/json" \
      #     -H "Accept: application/json;api-version=3.0-preview.1" \
      #     -d '{"filters":[{"criteria":[{"filterType":7,"value":"jnoortheen.nix-ide"}]}],"flags":403}' \
      #   | python3 -c "import json,sys; d=json.load(sys.stdin); \
      #     [print(v['version'], next((p['value'] for p in v.get('properties',[]) \
      #     if p['key']=='Microsoft.VisualStudio.Code.Engine'),'?')) \
      #     for v in d['results'][0]['extensions'][0]['versions']]"
      name      = "nix-ide";
      publisher = "jnoortheen";
      version   = "0.3.5";
      sha256    = "sha256-hiyFZVsZkxpc2Kh0zi3NGwA/FUbetAS9khWxYesxT4s=";
    }
    {
      name      = "direnv";
      publisher = "mkhl";
      version   = "0.17.0";
      sha256    = "sha256-9sFcfTMeLBGw2ET1snqQ6Uk//D/vcD9AVsZfnUNrWNg=";
    }
    {
      name      = "crates";
      publisher = "serayuzgur";
      version   = "0.6.7";
      sha256    = "sha256-FVZxMZ0QpCKLD0vX7LPvBywZgQ4kptjnCW9jCefwgJo=";
    }
    {
      name      = "even-better-toml";
      publisher = "tamasfe";
      version   = "0.19.2";
      sha256    = "sha256-JKj6noi2dTe02PxX/kS117ZhW8u7Bhj4QowZQiJKP2E=";
    }
  ];

  curated = with pkgs.vscode-extensions; [
    rust-lang.rust-analyzer
    vadimcn.vscode-lldb
    usernamehw.errorlens
    eamodio.gitlens
    streetsidesoftware.code-spell-checker
    editorconfig.editorconfig
  ];
in
curated ++ marketplaceExtras
