with (import <nixpkgs> { });

pkgs.mkShell {
  buildInputs = with pkgs; [
    nasm
    gcc
    gcc.libc
    gdb
    glibc.static
    linuxPackages.perf
    rustc
    cargo
    rustfmt
    rust-analyzer
    clippy
  ];
}
