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

        # Read version from workspace Cargo.toml — no manual bump needed.
        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake
        ];

        buildInputs = with pkgs; [
          openssl
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          # arboard (clipboard) needs X11 or Wayland on Linux
          libx11
          libxcursor
          libxrandr
          libxi
          libxkbcommon
          wayland
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.AppKit
          pkgs.darwin.apple_sdk.frameworks.Security
        ];

        orbt = pkgs.rustPlatform.buildRustPackage {
          pname = "orbt";
          inherit version;

          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;

          inherit nativeBuildInputs buildInputs;

          cargoBuildFlags = [ "-p" "orbt" "--bin" "orbt" ];
          cargoTestFlags = [ "-p" "orbt" "-p" "orbt-protocol" "-p" "orbt-core" ];

          meta = with pkgs.lib; {
            description = "Universal terminal workspace — sessions, panes, and AI agents";
            homepage = "https://github.com/linuszz/orbt";
            license = licenses.agpl3Only;
            mainProgram = "orbt";
            platforms = platforms.unix;
          };
        };
      in {
        packages = {
          default = orbt;
          orbt = orbt;
          orbit = orbt;   # alias
        };

        apps = {
          default = { type = "app"; program = "${orbt}/bin/orbt"; };
          orbt    = { type = "app"; program = "${orbt}/bin/orbt"; };
          orbit   = { type = "app"; program = "${orbt}/bin/orbt"; };
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
