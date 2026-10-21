# syntax=docker/dockerfile:1
# Build from the repository root: docker build -f docker/signaling-server/Dockerfile .
# The Rust server (crates/guardedsleep-server) on a distroless runtime. Multi-arch: build with
# `docker buildx build --platform linux/amd64,linux/arm64`; each platform compiles natively.
# Dockerfile reference: https://docs.docker.com/reference/dockerfile/

# --- toolchain: the channel comes from rust-toolchain.toml, the single source (ADR-020 section 1) ---
FROM debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad AS chef
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:${PATH}
RUN apt-get update \
	&& apt-get install -y --no-install-recommends build-essential ca-certificates curl \
	&& rm -rf /var/lib/apt/lists/*
COPY rust-toolchain.toml /tmp/rust-toolchain.toml
# Only the channel is installed here. The file itself is not copied into /app, so rustup does not pull
# the Android, iOS and WASM targets that the workspace toolchain file lists.
RUN CHANNEL="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' /tmp/rust-toolchain.toml)" \
	&& test -n "$CHANNEL" \
	&& curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$CHANNEL" \
	&& cargo install --locked cargo-chef@0.1.78 \
	&& rm -rf /opt/cargo/registry /opt/cargo/git /tmp/rust-toolchain.toml
WORKDIR /app

# --- planner: the dependency recipe, stable while only crate sources change ---
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo chef prepare --recipe-path recipe.json

# --- builder: dependencies first (cached layer), then the server ---
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json -p guardedsleep-server
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release --locked -p guardedsleep-server

# --- runtime: no shell, no package manager, non-root (uid 65532) ---
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:83b9b50586253821cf63ffcec87d3d94265a093ad30da232039af962dd8f4edf AS runner
COPY --from=builder /app/target/release/guardedsleep-server /usr/local/bin/guardedsleep-server

ENV PORT=8787
ENV STUN_PORT=3478
# Non-empty default so the process passes the production startup check; override in real deployments.
ENV NODE_ENV=production
ENV SIGNALING_ALLOWED_ORIGINS=http://127.0.0.1,http://localhost

USER nonroot
EXPOSE 8787/tcp 3478/udp

# The image has no curl or shell, so the binary probes its own /healthz.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
	CMD ["/usr/local/bin/guardedsleep-server", "--healthcheck"]

# Run with a read-only root filesystem (`docker run --read-only`): the server writes nothing.
ENTRYPOINT ["/usr/local/bin/guardedsleep-server"]
