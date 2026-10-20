# syntax=docker/dockerfile:1
# Static PWA (SvelteKit adapter-static) behind nginx. Build from repository root:
#   docker build -f docker/mobile-web/Dockerfile --build-arg VITE_SIGNALING_URL=wss://signal.example.com -t guardedsleep-web:local .
# Localization bundles come from the Rust generator (locales/*.json -> JSON), not from Node.
FROM rust:1-bookworm AS i18n
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY locales locales
RUN cargo run -q -p i18n-gen -- web --out /out

# WASM core (ADR-017): same steps as `scripts/core-build.sh wasm`, wasm-bindgen-cli pinned to Cargo.lock.
FROM rust:1-bookworm AS wasm
WORKDIR /src
RUN rustup target add wasm32-unknown-unknown
COPY Cargo.lock ./
RUN V="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/,""); print; exit}' Cargo.lock)" \
	&& cargo install --locked wasm-bindgen-cli --version "$V"
COPY Cargo.toml ./
COPY crates crates
RUN cargo build -p guardedsleep-core --release --target wasm32-unknown-unknown --features wasm \
	&& wasm-bindgen --target web --out-dir /out target/wasm32-unknown-unknown/release/guardedsleep_core.wasm

FROM node:22-alpine AS builder
WORKDIR /app

ENV HUSKY=0 CI=true
RUN corepack enable && corepack prepare pnpm@9.15.4 --activate

COPY pnpm-workspace.yaml package.json pnpm-lock.yaml ./
COPY packages/protocol/package.json packages/protocol/
COPY packages/core-wasm/package.json packages/core-wasm/
COPY apps/mobile/package.json apps/mobile/

RUN pnpm install --frozen-lockfile --ignore-scripts

COPY packages/protocol/tsconfig.json packages/protocol/tsconfig.build.json packages/protocol/
COPY packages/protocol/src packages/protocol/src
COPY packages/core-wasm/src packages/core-wasm/src
COPY --from=wasm /out packages/core-wasm/pkg
COPY apps/mobile apps/mobile
COPY --from=i18n /out apps/mobile/src/lib/i18n/generated

ARG VITE_SIGNALING_URL=
ENV VITE_SIGNALING_URL=$VITE_SIGNALING_URL

RUN pnpm --filter @guardedsleep/protocol build \
	&& pnpm --filter @guardedsleep/mobile exec vite build

FROM nginx:1.27-alpine AS runner

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

COPY docker/mobile-web/nginx.default.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/apps/mobile/build /usr/share/nginx/html

EXPOSE 80 443

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
	CMD wget -qO- http://127.0.0.1/ >/dev/null || exit 1
