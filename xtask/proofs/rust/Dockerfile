FROM docker.io/library/rust:1.99.1-trixie@sha256:767c71592f7d1f3f242292000f8efd4e67b6732363da176772f4d220332e264f AS worker

ENV CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
WORKDIR /source
COPY . .
RUN cargo build --locked --package xtask \
    && cp target/debug/xtask /usr/local/bin/sivi-proof-worker \
    && cargo fetch --locked --manifest-path fuzz/Cargo.toml \
    && cargo fetch --locked --manifest-path xtask/proofs/fuzz-fixture/Cargo.toml

FROM worker AS boundary

ENV CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/out/build TMPDIR=/out/tmp
RUN mkdir -p /out/tmp

FROM docker.io/library/rust:1.99.1-trixie@sha256:767c71592f7d1f3f242292000f8efd4e67b6732363da176772f4d220332e264f AS tools

ENV CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
RUN rustup toolchain install nightly-2026-08-21 --profile minimal --component miri --component rust-src \
    && rustup toolchain install 1.98.1 --profile minimal --component rustc-dev --component llvm-tools
RUN cargo install --locked kani-verifier --version 0.68.0 \
    && cargo kani setup \
    && cargo +nightly-2026-08-21 miri setup

ARG NEXTEST_SHA256=682c21b777c333e96fd532e114d3a5a894e0729ab88d94c0a9f20f8419695428
ADD --checksum=sha256:${NEXTEST_SHA256} https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.146/cargo-nextest-0.9.146-x86_64-unknown-linux-gnu.tar.gz /tmp/nextest.tar.gz
ARG FUZZ_SHA256=b5b704018b63e0f151c17a057ac53b5111e1db545d1b9f72fee79f08a545931c
ADD --checksum=sha256:${FUZZ_SHA256} https://github.com/rust-fuzz/cargo-fuzz/releases/download/0.13.2/cargo-fuzz-0.13.2-x86_64-unknown-linux-musl.tar.gz /tmp/fuzz.tar.gz
ARG MUTANTS_SHA256=dfe6dc37d0342c891d2829b5a695aa57c2d0edecef7e7d0399a30cc6e206411e
ADD --checksum=sha256:${MUTANTS_SHA256} https://github.com/sourcefrog/cargo-mutants/releases/download/v27.1.0/cargo-mutants-x86_64-unknown-linux-gnu.tar.gz /tmp/mutants.tar.gz
RUN tar -xzf /tmp/nextest.tar.gz -C /usr/local/cargo/bin \
    && tar -xzf /tmp/fuzz.tar.gz -C /usr/local/cargo/bin \
    && tar -xzf /tmp/mutants.tar.gz -C /usr/local/cargo/bin \
    && cargo nextest --version && cargo fuzz --version && cargo mutants --version \
    && rm /tmp/nextest.tar.gz /tmp/fuzz.tar.gz /tmp/mutants.tar.gz

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

FROM tools AS proofs

WORKDIR /source
COPY --from=worker /source /source
COPY --from=worker /usr/local/cargo/registry /usr/local/cargo/registry
COPY --from=worker /usr/local/bin/sivi-proof-worker /usr/local/bin/sivi-proof-worker
ENV CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/out/build TMPDIR=/out/tmp
RUN mkdir -p /out/tmp
