with import <nixpkgs> {};

with import <nixpkgs> {
  crossSystem = lib.systems.examples.riscv32-embedded;
};

with lib;

runCommand "dummy" rec {
  nativeBuildInputs = [
    gcc qemu
  ] ++ (optionals (!stdenv.isDarwin) [
    # needed by probe-rs
    libusb1
  ]);

  meta = {
    platforms = ["riscv32-none"];
  };
} ""
