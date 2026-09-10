/*
  Isolated VSCodium: our extension set baked in via pkgs.vscode-with-
  extensions, which wraps codium with --extensions-dir already pointed
  at an immutable Nix store path containing exactly our curated set
  from extensions.nix — fully isolated from any global VSCodium/VS
  Code profile by construction, and fully declarative: extensions are
  added by editing extensions.nix and reloading, not via the GUI
  Install button (that store path is read-only).

  We additionally set --user-data-dir to a project-local, writable
  folder — settings, recently-opened, workspace state — so that stays
  per-project without being baked into the Nix store.

  Exposed as both `vscodium` and `codium` — different platforms use
  either as the canonical binary name; both resolve to this instance.
*/
{ pkgs }:

let
  extensions = import ./extensions.nix { inherit pkgs; };

  vscodiumWithExtensions = pkgs.vscode-with-extensions.override {
    vscode           = pkgs.vscodium;
    vscodeExtensions = extensions;
  };

  defaultSettings = pkgs.writeText "settings.json" (builtins.toJSON {
    "rust-analyzer.server.path" = "${pkgs.rust-analyzer}/bin/rust-analyzer";
    "rust-analyzer.check.command" = "clippy";
    "rust-analyzer.cargo.features" = "all";
    "rust-analyzer.inlayHints.typeHints.enable" = true;
    "rust-analyzer.inlayHints.parameterHints.enable" = true;
    "rust-analyzer.inlayHints.chainingHints.enable" = true;

    "editor.formatOnSave" = true;
    "editor.inlayHints.enabled" = "on";
    "[rust]" = {
      "editor.defaultFormatter" = "rust-lang.rust-analyzer";
    };

    "files.watcherExclude" = {
      "**/target/**" = true;
      "**/.devenv/**" = true;
    };
    "files.exclude" = {
      "**/target" = true;
    };

    "lldb.executable" = "${pkgs.lldb}/bin/lldb";

    "nix.enableLanguageServer" = true;
    "nix.serverPath" = "${pkgs.nixd}/bin/nixd";
    "nix.formatterPath" = "${pkgs.alejandra}/bin/alejandra";
    "nix.serverSettings" = {
      "nixd" = {
        "formatting" = {
          "command" = [ "${pkgs.alejandra}/bin/alejandra" ];
        };
      };
    };
    "[nix]" = {
      "editor.defaultFormatter" = "jnoortheen.nix-ide";
      "editor.tabSize" = 2;
    };

    "telemetry.telemetryLevel" = "off";
    "update.mode" = "none";
    "extensions.autoCheckUpdates" = false;
    "extensions.autoUpdate" = false;
  });

  isolatedLauncher = name:
    pkgs.writeShellScriptBin name ''
      set -euo pipefail

      project_dir="$(pwd)"
      data_dir="''${project_dir}/.devenv/vscodium/user-data"

      mkdir -p "''${data_dir}/User"

      if [ ! -f "''${data_dir}/User/settings.json" ]; then
        cp ${defaultSettings} "''${data_dir}/User/settings.json"
        chmod u+w "''${data_dir}/User/settings.json"
      fi

      # Deliberately no --extensions-dir here: pkgs.vscode-with-extensions
      # already wraps codium with one baked in, pointing at the Nix store
      # path containing exactly our curated set from nix/extensions.nix.
      # That path is read-only by design — extensions are added by editing
      # extensions.nix and reloading, not via the GUI Install button.
      # Passing our own --extensions-dir here would silently override
      # theirs (VSCodium keeps the LAST of two conflicting flags) and
      # point at an empty, freshly-created folder — which is exactly the
      # "0 extensions installed" bug this comment is here to prevent
      # regressing.
      exec ${vscodiumWithExtensions}/bin/codium \
        --user-data-dir "''${data_dir}" \
        --disable-workspace-trust \
        "$@"
    '';
in
pkgs.symlinkJoin {
  name  = "vscodium-rust-isolated";
  paths = [
    (isolatedLauncher "vscodium")
    (isolatedLauncher "codium")
  ];
}
