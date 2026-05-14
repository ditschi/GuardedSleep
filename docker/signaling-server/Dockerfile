# syntax=docker/dockerfile:1
# Build from repository root: docker build -f docker/signaling-server/Dockerfile .
FROM node:22-alpine AS builder
WORKDIR /app

ENV HUSKY=0 CI=true
RUN corepack enable && corepack prepare pnpm@9.15.4 --activate

COPY pnpm-workspace.yaml package.json pnpm-lock.yaml ./
COPY packages/protocol/package.json packages/protocol/
COPY apps/signaling-server/package.json apps/signaling-server/

RUN pnpm install --frozen-lockfile --ignore-scripts

COPY packages/protocol/tsconfig.json packages/protocol/tsconfig.build.json packages/protocol/
COPY packages/protocol/src packages/protocol/src
COPY apps/signaling-server/tsconfig.json apps/signaling-server/
COPY apps/signaling-server/src ./apps/signaling-server/src

RUN pnpm --filter @guardedsleep/protocol build \
	&& pnpm --filter @guardedsleep/signaling-server build

RUN pnpm --filter @guardedsleep/signaling-server deploy /out --prod

FROM node:22-alpine AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV PORT=8787
# Non-empty default so the process passes startup checks; override in real deployments.
ENV SIGNALING_ALLOWED_ORIGINS=http://127.0.0.1,http://localhost

COPY --from=builder --chown=node:node /out/ ./

USER node

EXPOSE 8787

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
	CMD node -e "fetch('http://127.0.0.1:'+(process.env.PORT||8787)).then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"

CMD ["node", "dist/index.js"]
