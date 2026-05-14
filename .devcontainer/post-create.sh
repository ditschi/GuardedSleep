#!/usr/bin/env bash
set -euo pipefail

echo "==> Enabling Corepack (pnpm)"
sudo corepack enable
corepack prepare pnpm@9.15.4 --activate

echo "==> Installing pre-commit (user)"
python3 -m pip install --user --break-system-packages pre-commit 2>/dev/null \
  || python3 -m pip install --user pre-commit

export PATH="${HOME}/.local/bin:${PATH}"

echo "==> Installing Node devDependencies (commitlint, prettier for editor)"
if [[ -f package.json ]]; then
  pnpm install
fi

echo "==> Husky (Git hooks → .husky/; do not run pre-commit install — conflicts)"
export PATH="${HOME}/.local/bin:${PATH}"
chmod +x .husky/pre-commit .husky/commit-msg 2>/dev/null || true
# prepare script runs husky after pnpm install; ensures core.hooksPath
pnpm run prepare 2>/dev/null || true

echo "==> Optional: run pre-commit once on all files (may take a minute)"
pre-commit run --all-files

echo "==> Devcontainer ready. Tools: node $(node -v), pnpm $(pnpm -v), pre-commit $(pre-commit --version)"
