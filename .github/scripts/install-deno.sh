#!/bin/sh

# Installs a binary distribution of Deno. (The Snapcraft version of Deno seems
# unable to locate `cargo`.)

version=1.32.1
wget https://github.com/denoland/deno/releases/download/v${version}/deno-x86_64-unknown-linux-gnu.zip
unzip deno-x86_64-unknown-linux-gnu.zip
chmod +x deno
mkdir deno-${version}
mv deno deno-${version}
echo $PWD/deno-${version} >> $GITHUB_PATH
