# syntax=docker/dockerfile:1
# Static PWA (SvelteKit adapter-static) behind nginx. Build from repository root:
#   docker build -f docker/mobile-web/Dockerfile --build-arg VITE_SIGNALING_URL=wss://signal.example.com -t guardedsleep-web:local .
FROM node:22-alpine AS builder
WORKDIR /app

ENV HUSKY=0 CI=true
RUN corepack enable && corepack prepare pnpm@9.15.4 --activate

COPY pnpm-workspace.yaml package.json pnpm-lock.yaml ./
COPY packages/protocol/package.json packages/protocol/
COPY apps/mobile/package.json apps/mobile/

RUN pnpm install --frozen-lockfile --ignore-scripts

COPY packages/protocol/tsconfig.json packages/protocol/tsconfig.build.json packages/protocol/
COPY packages/protocol/src packages/protocol/src
COPY apps/mobile apps/mobile

ARG VITE_SIGNALING_URL=
ENV VITE_SIGNALING_URL=$VITE_SIGNALING_URL

RUN pnpm --filter @guardedsleep/protocol build \
	&& pnpm --filter @guardedsleep/mobile build

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
