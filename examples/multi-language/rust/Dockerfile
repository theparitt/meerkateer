FROM rust:1.93.1-bookworm@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31 AS builder
WORKDIR /source
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY sdk/rust ./sdk/rust
COPY examples/multi-language/rust ./examples/multi-language/rust
RUN --mount=type=cache,id=meerkateer-demo-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=meerkateer-demo-rust-target,target=/source/examples/multi-language/rust/target \
    cargo build --locked --release --manifest-path examples/multi-language/rust/Cargo.toml \
    && cp examples/multi-language/rust/target/release/meerkateer-rust-demo-server /demo-server

FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818
RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /demo-server /demo-server
USER 65532:65532
ENTRYPOINT ["/demo-server"]
