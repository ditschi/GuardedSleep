# Dev: Android SDK/NDK + Rust (core, WASM, Android) + mise in one image. Node, pnpm, just and the other
# pinned tools come from mise.toml (`mise install`, run by post-create.sh); the JDK is Temurin 21.
# Bookworm often has no OpenJDK 21 in default APT, so we copy Temurin from the official image
# (the same one docker/android-native-build uses).
# Build context: .devcontainer/ plus the named context "repo" (repo root, for rust-toolchain.toml and tools/android-versions.env):
#   docker-compose.yml needs build.additional_contexts: { repo: .. }
FROM eclipse-temurin:21-jdk-jammy@sha256:742daacb9316be4b6fc60258f6e4ac2a11882f6c597373c92290a57bfad0d2e0 AS temurin21

FROM mcr.microsoft.com/devcontainers/typescript-node:1-22-bookworm@sha256:7c2e711a4f7b02f32d2da16192d5e05aa7c95279be4ce889cff5df316f251c1d

USER root

COPY --from=temurin21 /opt/java/openjdk /opt/java/openjdk

ENV JAVA_HOME=/opt/java/openjdk
ENV ANDROID_HOME=/opt/android-sdk
ARG ANDROID_CLI_TOOLS_URL=https://dl.google.com/android/repository/commandlinetools-linux-16111833_latest.zip
# NDK and cargo-ndk versions come from tools/android-versions.env (named context "repo"); the symlink
# below gives the NDK a stable path.
ENV ANDROID_NDK_HOME=/opt/android-sdk/ndk/pinned
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
# scripts/core-build.sh and scripts/gradle.sh run natively here (no nested Docker).
ENV CORE_BUILD_NATIVE=1

COPY --from=repo tools/android-versions.env /tmp/android-versions.env

RUN . /tmp/android-versions.env \
	&& apt-get update && export DEBIAN_FRONTEND=noninteractive \
	&& apt-get install -y --no-install-recommends \
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
	&& ln -s "${ANDROID_HOME}/ndk/${ANDROID_NDK_VERSION}" "${ANDROID_HOME}/ndk/pinned" \
	&& chown -R node:node "${ANDROID_HOME}" \
	&& apt-get clean \
	&& rm -rf /var/lib/apt/lists/*

# Rust (ADR-015/019): core, WASM and Android targets, cargo-ndk; mirrors docker/android-native-build.
# Installed system-wide so the `node` user (and the rootless uid-0 mapping) can both use it.
ENV PATH="/opt/cargo/bin:${PATH}"
COPY --from=repo rust-toolchain.toml /tmp/rust-toolchain/rust-toolchain.toml
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none \
	&& (cd /tmp/rust-toolchain && rustup toolchain install \
		&& rustup default "$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)") \
	&& (. /tmp/android-versions.env && cargo install --locked "cargo-ndk@${CARGO_NDK_VERSION}") \
	&& chmod -R a+rwX /opt/cargo /opt/rustup \
	&& rm -rf /opt/cargo/registry /opt/cargo/git

# mise installs the tools pinned in mise.toml (ADR-020 section 1); its shims put them on PATH for every
# shell. The installer verifies the release checksum. https://mise.jdx.dev/installing-mise.html
ARG MISE_VERSION=v2026.10.15
RUN curl -fsSL https://mise.run | MISE_VERSION="${MISE_VERSION}" MISE_INSTALL_PATH=/usr/local/bin/mise sh
ENV PATH="/home/node/.local/share/mise/shims:${PATH}"

ENV PATH="${PATH}:${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools"

# Login / interactive shells (nvm) often reset PATH — re-export after profile runs.
RUN printf '%s\n' \
	'export JAVA_HOME=/opt/java/openjdk' \
	'export ANDROID_HOME=/opt/android-sdk' \
	'export PATH="/home/node/.local/share/mise/shims:/opt/cargo/bin:${PATH}"' \
	'PATH="${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${PATH}"' \
	'export PATH' \
	> /etc/profile.d/guardedsleep-android.sh \
	&& chmod 644 /etc/profile.d/guardedsleep-android.sh

RUN printf '\n# GuardedSleep Android (after nvm in .bashrc)\nexport JAVA_HOME=/opt/java/openjdk\nexport ANDROID_HOME=/opt/android-sdk\nexport PATH="/home/node/.local/share/mise/shims:/opt/cargo/bin:${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${PATH}"\n' >> /home/node/.bashrc \
	&& chown node:node /home/node/.bashrc

USER node
WORKDIR /workspace
