{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=master";

    rust = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    treefmt = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust,
      devshell,
      treefmt,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;

        overlays = [
          (import rust)
          devshell.overlays.default
        ];
      };

      crossPkgs = {
        aarch64-embedded = import nixpkgs {
          localSystem = system;
          crossSystem = {
            config = "aarch64-unknown-none-elf";
            rust.rustcTarget = "aarch64-unknown-none";
          };
        };
      };

      treefmtEval = treefmt.lib.evalModule pkgs ./treefmt.nix;
    in
    {
      packages.${system} = {
      };

      devShells.${system}.default =
        let
          rust-toolchain = pkgs.rust-bin.selectLatestNightlyWith (
            toolchain:
            toolchain.default.override {
              extensions = [
                "rust-src"
                "rustfmt"
                "rust-analyzer"
              ];
              targets = [
                "armv7a-none-eabi"
                "aarch64-unknown-none"
              ];
            }
          );
        in
        pkgs.devshell.mkShell {
          imports = [ "${devshell}/extra/git/hooks.nix" ];

          packages = [
            pkgs.gdb
            pkgs.stdenv.cc
            crossPkgs.aarch64-embedded.stdenv.cc
            rust-toolchain
            pkgs.qemu_full
          ];

          git.hooks = {
            enable = true;
            pre-commit.text = ''
              nix fmt
              nix flake check
            '';
          };
        };

      # for `nix fmt`
      formatter.${system} = treefmtEval.config.build.wrapper;

      # for `nix flake check`
      checks.${system}.formatting = treefmtEval.config.build.check self;
    };
}
