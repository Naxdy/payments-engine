{ pkgs, ... }:
let
  inherit (pkgs.payments-engine.passthru) cargoToml rustToolchain;
in
{
  # rust
  programs.rustfmt = {
    enable = true;
    package = rustToolchain;
    edition = cargoToml.workspace.package.edition or cargoToml.package.edition;
  };

  # nix
  programs.nixfmt.enable = true;

  # toml
  programs.taplo.enable = true;

  # markdown, yaml, etc.
  programs.prettier = {
    enable = true;
    settings = {
      trailingComma = "all";
      semi = true;
      printWidth = 120;
      singleQuote = true;
      proseWrap = "always";
    };
  };

  programs.typos = {
    enable = true;
    includes = [
      "*.md"
      "*.nix"
      "*.rs"
    ];
  };
}
