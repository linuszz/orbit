{
  description = "Orbt — universal terminal workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default;

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake
        ];

        buildInputs = with pkgs; [
          openssl
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
          libxkbcommon
          wayland
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.AppKit
          pkgs.darwin.apple_sdk.frameworks.Security
        ];
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "orbt";
          version = "0.1.6";

          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;

          inherit nativeBuildInputs buildInputs;

          # Only build the orbt binary
          cargoBuildFlags = [ "-p" "orbt" "--bin" "orbt" ];
          cargoTestFlags = [ "-p" "orbt" "-p" "orbt-protocol" "-p" "orbt-core" ];

          meta = with pkgs.lib; {
            description = "Universal terminal workspace — sessions, panes, and AI agents";
            homepage = "https://github.com/linuszz/orbt";
            license = licenses.agpl3Only;
            mainProgram = "orbt";
          };
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
            rust-analyzer
            clippy
          ]);
        };
      }
    );
}
