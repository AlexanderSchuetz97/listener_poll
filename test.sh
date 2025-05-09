#!/usr/bin/env bash
docker run --rm -it -v $(pwd):/io alpine:latest rm -rf /io/target
cargo clean

set -e
rustup default stable

cargo clippy --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy --target x86_64-pc-windows-gnu -- -D warnings
cargo clippy --target x86_64-unknown-linux-musl -- -D warnings
cargo clippy --target i686-unknown-linux-musl -- -D warnings

# 64 bit little endian
cross test --target x86_64-unknown-linux-gnu
cross test --target x86_64-unknown-linux-musl

# 32 bit little endian
cross test --target i686-unknown-linux-gnu
cross test --target i686-unknown-linux-musl

#64 bit big endian
cross test --target s390x-unknown-linux-gnu

#32 bit big endian
cross test --target powerpc-unknown-linux-gnu

# time_t is weird on sparc for some reason.
cross test --target sparc64-unknown-linux-gnu

#Windows (this test requires mingw and wine and wine-binfmt to work)
cargo test --target x86_64-pc-windows-gnu
cargo test --target i686-pc-windows-gnu

# Mac
docker run --rm --net host -it -v $(pwd):/io -w /io ghcr.io/rust-cross/cargo-zigbuild cargo zigbuild --target universal2-apple-darwin

# BSD
cross build --target x86_64-unknown-netbsd
cross build --target x86_64-unknown-freebsd

# Cleanup
docker run --rm -it -v $(pwd):/io alpine:latest rm -rf /io/target
cargo clean

# Verify minimum rust version works
cargo msrv verify --ignore-lockfile