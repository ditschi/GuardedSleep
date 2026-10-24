# syntax=docker/dockerfile:1
# Static PWA (apps/web: Vite + Svelte 5, no SvelteKit) behind nginx. Build from repository root:
#   docker build -f docker/web/Dockerfile -t guardedsleep-web:local .
# The PWA infers its signaling URL from the page origin (nginx proxies /signaling), so there is no
# build-time signaling URL.
# Localization bundles come from the Rust generator (locales/*.json -> JSON), not from Node.
FROM rust:1-bookworm@sha256:665b6e0a9290a7298236c08c6e4f1e754e3f0d9b70421c434daa3dfba888d63e AS i18n
WORKDIR /src
# The Rust toolchain comes from rust-toolchain.toml (ADR-020 section 1), not from the base image's Rust.
COPY rust-toolchain.toml ./
RUN rustup toolchain install
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY locales locales
RUN cargo run -q -p i18n-gen -- web --out /out

# WASM core (ADR-017): same steps as `scripts/core-build.sh wasm`, wasm-bindgen-cli pinned to Cargo.lock.
FROM rust:1-bookworm@sha256:665b6e0a9290a7298236c08c6e4f1e754e3f0d9b70421c434daa3dfba888d63e AS wasm
WORKDIR /src
COPY rust-toolchain.toml ./
RUN rustup toolchain install
COPY Cargo.lock ./
RUN V="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/,""); print; exit}' Cargo.lock)" \
	&& cargo install --locked wasm-bindgen-cli --version "$V"
COPY Cargo.toml ./
COPY crates crates
RUN cargo build -p guardedsleep-wasm --release --target wasm32-unknown-unknown \
	&& wasm-bindgen --target web --out-dir /out target/wasm32-unknown-unknown/release/guardedsleep_wasm.wasm

FROM node:22-alpine@sha256:0a7108bf6c7bf5de370ffb1a3ed6be93d405b43ff159f681a8d18c0e2bc2e402 AS builder
WORKDIR /app

ENV CI=true
# pnpm 11 at the version pinned in mise.toml (the single source; corepack is not used, ADR-020 section 5).
# https://pnpm.io/installation
COPY mise.toml /tmp/mise.toml
RUN PNPM_VERSION="$(sed -n 's/^pnpm *= *"\(.*\)"/\1/p' /tmp/mise.toml)" \
	&& test -n "$PNPM_VERSION" \
	&& npm install --global "pnpm@${PNPM_VERSION}" \
	&& rm /tmp/mise.toml

COPY pnpm-workspace.yaml package.json pnpm-lock.yaml ./
COPY packages/core-wasm/package.json packages/core-wasm/
COPY apps/web/package.json apps/web/

RUN pnpm install --frozen-lockfile --ignore-scripts

COPY packages/core-wasm/src packages/core-wasm/src
COPY --from=wasm /out packages/core-wasm/pkg
COPY apps/web apps/web
COPY --from=i18n /out apps/web/src/lib/i18n/generated

# `vite build` directly: the package's `prebuild` hook would run cargo, and both generated inputs
# (i18n, WASM core) come from the stages above.
RUN pnpm --filter @guardedsleep/web exec vite build

FROM nginx:1.27-alpine@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10 AS runner

# Optional build-arg, e.g. IP:192.168.1.42 — adds that name to the self-signed cert (better match when opening https://192.168…).
ARG TLS_EXTRA_SAN=
RUN apk add --no-cache openssl \
	&& mkdir -p /etc/nginx/ssl \
	&& SAN="DNS:localhost,IP:127.0.0.1" \
	&& if [ -n "${TLS_EXTRA_SAN}" ]; then SAN="${SAN},${TLS_EXTRA_SAN}"; fi \
	&& openssl req -x509 -nodes -days 825 -newkey rsa:2048 \
		-keyout /etc/nginx/ssl/selfsigned.key \
		-out /etc/nginx/ssl/selfsigned.crt \
		-subj "/CN=localhost" \
		-addext "subjectAltName=$SAN"

COPY docker/web/nginx.default.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/apps/web/dist /usr/share/nginx/html

EXPOSE 80 443

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
	CMD wget -qO- http://127.0.0.1/ >/dev/null || exit 1
