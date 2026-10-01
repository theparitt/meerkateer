# syntax=docker/dockerfile:1.7
FROM rust:1.93.1-bookworm@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31 AS builder
WORKDIR /source
ENV RUSTUP_TOOLCHAIN=1.93.1
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY meerkateer-agent ./meerkateer-agent
COPY meerkateer-server ./meerkateer-server
COPY meerkateer-worker ./meerkateer-worker
COPY openapi ./openapi
COPY sdk/rust ./sdk/rust
RUN --mount=type=cache,id=meerkateer-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=meerkateer-release-target,target=/source/target \
    cargo build --locked --release -p meerkateer-server -p meerkateer-worker \
    && mkdir -p /out \
    && cp /source/target/release/meerkateer-server /out/meerkateer-server \
    && cp /source/target/release/meerkateer-worker /out/meerkateer-worker

FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 AS runtime
RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 meerkateer \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin meerkateer
COPY --from=builder /out/meerkateer-server /usr/local/bin/meerkateer-server
COPY --from=builder /out/meerkateer-worker /usr/local/bin/meerkateer-worker
USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/meerkateer-server"]
