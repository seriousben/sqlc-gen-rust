{
  description = "sqlc-gen-rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          nativeBuildInputs = with pkgs; [ rustToolchain protobuf_28 ];
          buildInputs = with pkgs; [ openssl ];
        in
        with pkgs;
        {
          packages.default = pkgs.rustPlatform.buildRustPackage {
            inherit system nativeBuildInputs buildInputs;
            name = "sqlc-gen-rust";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            # see https://www.tweag.io/blog/2022-09-22-rust-nix/
            buildPhase = ''
              cargo build --release -p sqlc-gen-rust --target=wasm32-wasip1
            '';

            installPhase = ''
              mkdir -p $out/bin
              cp target/wasm32-wasip1/release/*.wasm $out/bin/
              touch $out/bin/sqlc-gen-rust.wasm
              ls -la $out/bin > $out/build.log
            '';
          };

          devShells.default = mkShell {
            inherit nativeBuildInputs;
            buildInputs = buildInputs ++ [ rust-analyzer sqlc wasmtime ];
          };
        }
      );
}
