{ pkgs ? import <nixpkgs> {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  }
}:

let
  rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustToolchain
    pkg-config
    openssl
    glib
    gexiv2
    sqlite
  ];

  RUST_BACKTRACE = 1;
}