# Dev: Node 22 + Temurin JDK 21 + Android SDK/NDK + Rust (core, WASM, Android) + just in one image.
# JDK 21 matches Capacitor/Android `source release 21`. Bookworm often has no OpenJDK 21 in default APT,
# so we copy Temurin from the official image (same family as docker/android-build).
# Build context: .devcontainer/ plus the named context "repo" (repo root, for rust-toolchain.toml):
#   docker-compose.yml needs build.additional_contexts: { repo: .. }
FROM eclipse-temurin:21-jdk-jammy AS temurin21

FROM mcr.microsoft.com/devcontainers/typescript-node:1-22-bookworm

USER root

COPY --from=temurin21 /opt/java/openjdk /opt/java/openjdk

ENV JAVA_HOME=/opt/java/openjdk
ENV ANDROID_HOME=/opt/android-sdk
ARG ANDROID_CLI_TOOLS_URL=https://dl.google.com/android/repository/commandlinetools-linux-16111833_latest.zip
ARG ANDROID_NDK_VERSION=30.0.16248370
ENV ANDROID_NDK_HOME=/opt/android-sdk/ndk/${ANDROID_NDK_VERSION}
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
# scripts/core-build.sh and scripts/gradle.sh run natively here (no nested Docker).
ENV CORE_BUILD_NATIVE=1

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
	&& apt-get install -y --no-install-recommends \
		python3 \
		python3-pip \
		gh \
		unzip \
		wget \
		build-essential \
	&& mkdir -p "${ANDROID_HOME}/cmdline-tools" \
	&& wget -q -O /tmp/cmdline-tools.zip "${ANDROID_CLI_TOOLS_URL}" \
	&& unzip -q /tmp/cmdline-tools.zip -d "${ANDROID_HOME}/cmdline-tools" \
	&& mv "${ANDROID_HOME}/cmdline-tools/cmdline-tools" "${ANDROID_HOME}/cmdline-tools/latest" \
	&& rm -f /tmp/cmdline-tools.zip \
	&& yes | "${ANDROID_HOME}/cmdline-tools/latest/bin/sdkmanager" --licenses >/dev/null \
	&& "${ANDROID_HOME}/cmdline-tools/latest/bin/sdkmanager" --install \
		"platform-tools" \
		"platforms;android-35" \
		"build-tools;35.0.0" \
		"platforms;android-37.0" \
		"build-tools;36.0.0" \
		"ndk;${ANDROID_NDK_VERSION}" \
	&& chown -R node:node "${ANDROID_HOME}" \
	&& apt-get clean \
	&& rm -rf /var/lib/apt/lists/*

# System-wide so `git commit` works when the compose user is root (rootless uid map);
# post-create still installs to ~/.local for the node user, but hooks must find pre-commit on PATH.
RUN python3 -m pip install --no-cache-dir --break-system-packages pre-commit 2>/dev/null \
	|| python3 -m pip install --no-cache-dir pre-commit

# Rust (ADR-015/019): core, WASM and Android targets, cargo-ndk, `just`; mirrors docker/android-native-build.
# Installed system-wide so the `node` user (and the rootless uid-0 mapping) can both use it.
ENV PATH="/opt/cargo/bin:${PATH}"
COPY --from=repo rust-toolchain.toml /tmp/rust-toolchain/rust-toolchain.toml
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none \
	&& (cd /tmp/rust-toolchain && rustup toolchain install \
		&& rustup default "$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)") \
	&& cargo install --locked cargo-ndk@4.1.2 \
	&& curl -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin \
	&& chmod -R a+rwX /opt/cargo /opt/rustup \
	&& rm -rf /opt/cargo/registry /opt/cargo/git

# @cspell 10.x requires Node >= 22.18; the devcontainer base image may ship an older patch.
ARG NODE_FULL_VERSION=22.18.0
RUN set -eu \
	&& ARCH="$(uname -m)" \
	&& case "$ARCH" in \
		x86_64) NODE_ARCH=x64 ;; \
		aarch64) NODE_ARCH=arm64 ;; \
		*) echo "unsupported arch for Node bump: $ARCH" >&2; exit 1 ;; \
	esac \
	&& curl -fsSL "https://nodejs.org/dist/v${NODE_FULL_VERSION}/node-v${NODE_FULL_VERSION}-linux-${NODE_ARCH}.tar.xz" \
		| tar -xJ -C /usr/local --strip-components=1 \
		--exclude CHANGELOG.md --exclude LICENSE --exclude README.md

ENV PATH="${PATH}:${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools"

# Login / interactive shells (nvm) often reset PATH — re-export after profile runs.
RUN printf '%s\n' \
	'export JAVA_HOME=/opt/java/openjdk' \
	'export ANDROID_HOME=/opt/android-sdk' \
	'export PATH="/opt/cargo/bin:${PATH}"' \
	'PATH="${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${PATH}"' \
	'export PATH' \
	> /etc/profile.d/guardedsleep-android.sh \
	&& chmod 644 /etc/profile.d/guardedsleep-android.sh

RUN printf '\n# GuardedSleep Android (after nvm in .bashrc)\nexport JAVA_HOME=/opt/java/openjdk\nexport ANDROID_HOME=/opt/android-sdk\nexport PATH="/opt/cargo/bin:${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${PATH}"\n' >> /home/node/.bashrc \
	&& chown node:node /home/node/.bashrc

USER node
WORKDIR /workspace
