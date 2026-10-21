# Host-independent browser E2E runner (Playwright + Chromium + system deps preinstalled).
# Build context: docker/e2e-local/
FROM mcr.microsoft.com/playwright:v1.60.0-noble@sha256:9bd26ad900bb5e0f4dee75839e957a89ae89c2b7ab1e76050e559790e946b948

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

# pnpm is installed by the entrypoint at the version pinned in /workspace/mise.toml (this build context is
# docker/e2e-local, which cannot see the repo root).

WORKDIR /workspace

COPY entrypoint.sh /usr/local/bin/e2e-entrypoint.sh
RUN chmod +x /usr/local/bin/e2e-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/e2e-entrypoint.sh"]
