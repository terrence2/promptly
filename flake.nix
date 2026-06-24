{
  description = "eframe devShell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        toolchain = (builtins.fromTOML (builtins.readFile ("${self}/rust-toolchain.toml")));
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
      in with pkgs; {
        devShells.default = mkShell rec {
          # Present at both build and run time
          buildInputs = [
            # Rust
            (rust-bin.stable.${toolchain.toolchain.channel}.default.override {
              extensions = toolchain.toolchain.components;
              targets = toolchain.toolchain.targets;
            })
          ];

          # Build only, not present at runtime
          nativeBuildInputs = [
            clang
          ];

          packages = [
          ];

          LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
        };
      });
}
