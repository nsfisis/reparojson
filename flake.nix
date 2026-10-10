{
  description = "Command-line tool to repair syntactic errors in JSON without formatting it";

  inputs = {
    self.submodules = true;

    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    systems.url = "github:nix-systems/default";

    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      treefmt-nix,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        treefmt = treefmt-nix.lib.evalModule pkgs {
          imports = [ ./treefmt.nix ];
          programs.rustfmt.package = rustToolchain;
        };

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        reparojson = rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          inherit (cargoToml.package) version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [ pkgs.installShellFiles ];

          postInstall = pkgs.lib.optionalString (pkgs.stdenv.buildPlatform.canExecute pkgs.stdenv.hostPlatform) ''
            installShellCompletion --cmd reparojson \
              --bash <($out/bin/reparojson --generate-completion bash) \
              --fish <($out/bin/reparojson --generate-completion fish) \
              --zsh <($out/bin/reparojson --generate-completion zsh)
          '';

          meta = {
            description = "Command-line tool to repair syntactic errors in JSON without formatting it";
            homepage = "https://github.com/nsfisis/reparojson";
            license = pkgs.lib.licenses.mit;
            mainProgram = "reparojson";
          };
        };
      in
      {
        formatter = treefmt.config.build.wrapper;

        packages = {
          inherit reparojson;
          default = reparojson;
        };

        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
          ];
        };
      }
    );
}
