with import <nixpkgs> {};
with lib;

runCommand "dummy" rec {
  nativeBuildInputs = [
    rustup pkgconfig gcc
  ];

  # TODO: why not `(!stdenv.isDarwin)`?
  buildInputs = optionals (!stdenv.isDarwin) [
    # needed by probe-rs
    libusb1

    # needed by `constance_test_runner`
    qemu
    openocd
  ];
} ""
