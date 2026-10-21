FROM docker.io/library/rust:1.99.1-trixie@sha256:767c71592f7d1f3f242292000f8efd4e67b6732363da176772f4d220332e264f

ENV CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
RUN rustup toolchain install nightly-2026-08-21 --profile minimal --component miri --component rust-src \
    && rustup toolchain install 1.98.1 --profile minimal --component rustc-dev --component llvm-tools
RUN cargo install --locked kani-verifier --version 0.68.0 \
    && cargo kani setup \
    && cargo install --locked cargo-nextest --version 0.9.146 \
    && cargo install --locked cargo-fuzz --version 0.13.2 \
    && cargo install --locked cargo-mutants --version 27.1.0 \
    && cargo +nightly-2026-08-21 miri setup

ARG VERUS_SHA256=624018662868563a7bec0dd07be5e4dd0c3d61124b4b55105765884bf2d4d1a0
ADD --checksum=sha256:${VERUS_SHA256} https://github.com/verus-lang/verus/releases/download/release/0.2026.10.04.426d8b0/verus-0.2026.10.04.426d8b0-x86-linux.zip /tmp/verus.zip
ARG Z3_SHA256=f47be8d27d3230e823bf1eeede2fe0abaca55bb78d0b59974370e6689a92284a
ADD --checksum=sha256:${Z3_SHA256} https://github.com/Z3Prover/z3/releases/download/z3-5.1.0/z3-5.1.0-x64-glibc-2.39.zip /tmp/z3.zip
RUN apt-get update && apt-get install --no-install-recommends -y unzip \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /opt/sivi/verus \
    && unzip -q /tmp/verus.zip -d /opt/sivi/verus \
    && unzip -q /tmp/z3.zip -d /opt \
    && ln -s /opt/z3-5.1.0-x64-glibc-2.39/bin/z3 /usr/local/bin/z3 \
    && rm /tmp/verus.zip /tmp/z3.zip

WORKDIR /source
COPY . .
RUN cargo fetch --locked \
    && cargo +nightly-2026-08-21 fetch --locked --manifest-path fuzz/Cargo.toml \
    && cargo +nightly-2026-08-21 fetch --locked --manifest-path xtask/proofs/fuzz-fixture/Cargo.toml \
    && cargo build --locked --package xtask \
    && cp target/debug/xtask /usr/local/bin/sivi-proof-worker

ENV CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/out/build TMPDIR=/out/tmp
RUN mkdir -p /out/tmp
