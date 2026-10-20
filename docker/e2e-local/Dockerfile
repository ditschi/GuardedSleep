# Host-independent browser E2E runner (Playwright + Chromium + system deps preinstalled).
# Build context: docker/e2e-local/
FROM mcr.microsoft.com/playwright:v1.60.0-noble

USER root

RUN apt-get update \
	&& apt-get install -y --no-install-recommends \
		docker.io \
		curl \
		ca-certificates \
		iproute2 \
	&& rm -rf /var/lib/apt/lists/* \
	&& mkdir -p /usr/local/lib/docker/cli-plugins \
	&& ARCH="$(uname -m)" \
	&& curl -fsSL "https://github.com/docker/compose/releases/download/v2.32.4/docker-compose-linux-${ARCH}" \
		-o /usr/local/lib/docker/cli-plugins/docker-compose \
	&& chmod +x /usr/local/lib/docker/cli-plugins/docker-compose

RUN corepack enable && corepack prepare pnpm@9.15.4 --activate

WORKDIR /workspace

COPY entrypoint.sh /usr/local/bin/e2e-entrypoint.sh
RUN chmod +x /usr/local/bin/e2e-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/e2e-entrypoint.sh"]
