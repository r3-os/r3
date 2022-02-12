with import <nixpkgs> {};
with lib;

runCommand "dummy" rec {
  buildInputs = [
    pkgconfig
    libusb1
    openocd
    rustup
    tagref
    qemu
    gcc
  ];
} ""
