FROM rust:1.88-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
# Stub enterprise crate so Cargo can resolve the optional path dependency
RUN mkdir -p enterprise/src && \
    echo '[package]\nname = "tracevault-enterprise"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\ntracevault-core = { path = "../crates/tracevault-core" }' > enterprise/Cargo.toml && \
    echo '' > enterprise/src/lib.rs
RUN cargo build --release -p tracevault-server

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 git && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/tracevault-server /usr/local/bin/
ENV PORT=3000
ENV HOST=0.0.0.0
ENV TRACEVAULT_REPOS_DIR=/data/repos
EXPOSE 3000
CMD ["tracevault-server"]
