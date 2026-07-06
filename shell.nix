{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
  ];

  buildInputs = with pkgs; [
    gcc
    openssl
  ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
    darwin.libutil
  ];

  shellHook = ''
    export CC=gcc
  '';
}
