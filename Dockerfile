ARG DEBIAN_VERSION=trixie

# Stage 1: Build
FROM rust:slim-${DEBIAN_VERSION} AS builder
WORKDIR /app

RUN rm -f /etc/apt/apt.conf.d/docker-clean; echo 'Binary::apt::APT::Keep-Downloaded-Packages "true";' > /etc/apt/apt.conf.d/keep-cache
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y \
    pkg-config libssl-dev libsystemd-dev \
    build-essential perl

COPY ./Cargo.lock Cargo.lock
COPY ./Cargo.toml Cargo.toml
COPY ./ivaldi-cli ivaldi-cli
COPY ./ivaldi-core ivaldi-core
COPY ./ivaldi-server ivaldi-server

# Build all workspace members (cli and server)
# Build all workspace members (cli and server)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

# Stage 2: Runtime
FROM debian:${DEBIAN_VERSION}-slim
# Install libssl or other runtime deps if needed
# Install libssl or other runtime deps if needed
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y ca-certificates

WORKDIR /
COPY --from=builder /app/target/release/ivaldi-server /usr/local/bin/ivaldi-server
COPY --from=builder /app/target/release/ivaldi /usr/local/bin/ivaldi

# Default environment variables
ENV IVALDI_LOG=info
ENV IVALDI_ROOT=/projects
ENV IVALDI_API_KEY=""
ENV IVALDI_CONFIG=""
ENV IVALDI_VECDB_URL="http://localhost:8080"

# The ENTRYPOINT is the binary itself
ENTRYPOINT ["/usr/local/bin/ivaldi-server"]