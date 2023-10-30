{ mkShell
, rust-analyzer
, rustc
, rustfmt
, cargo
, cargo-tarpaulin
, clippy
, openssl
, pkg-config
, sqlite
, ...
}:

mkShell {
  nativeBuildInputs = [
    rust-analyzer
    rustc
    rustfmt
    cargo
    cargo-tarpaulin
    clippy
    openssl
    pkg-config
    sqlite
  ];
}
