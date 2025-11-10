{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    fenix.url = "github:nix-community/fenix";

    crane.url = "github:ipetkov/crane";

    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      crane,
      treefmt-nix,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          let
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };

            treefmtEval = treefmt-nix.lib.evalModule pkgs ./treefmt.nix;

            treefmt = treefmtEval.config.build.wrapper;
          in
          f {
            inherit
              pkgs
              system
              treefmt
              treefmtEval
              ;
          }
        );
    in
    {
      devShells = forEachSupportedSystem (
        {
          pkgs,
          treefmt,
          system,
          ...
        }:
        {
          default = self.devShells.${system}.full;

          full = pkgs.mkShell {
            packages = [
              treefmt
            ];

            inputsFrom = [ self.packages.${system}.default ];
          };

          toolchainOnly = pkgs.mkShell {
            nativeBuildInputs = [
              self.packages.${system}.default.passthru.rustToolchain
            ];
          };
        }
      );

      formatter = forEachSupportedSystem ({ treefmt, ... }: treefmt);

      packages = forEachSupportedSystem (
        {
          pkgs,
          system,
          ...
        }:
        {
          default = self.packages.${system}.payments-engine;

          payments-engine = pkgs.payments-engine;

          inherit (pkgs.payments-engine) docs;
        }
      );

      checks = forEachSupportedSystem (
        {
          pkgs,
          treefmtEval,
          system,
          ...
        }:
        let
          testsFrom =
            pkg:
            pkgs.lib.mapAttrs' (name: value: {
              name = "${pkg.pname}-${name}";
              inherit value;
            }) pkg.passthru.tests;
        in
        {
          treefmt = treefmtEval.config.build.check self;
        }
        // (testsFrom self.packages.${system}.default)
      );

      overlays.default = final: prev: {
        payments-engine = final.callPackage ./default.nix {
          craneLib = crane.mkLib final;
          fenix = final.callPackage fenix { };
        };
      };
    };
}
