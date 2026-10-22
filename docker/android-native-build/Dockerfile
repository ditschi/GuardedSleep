# JDK 21 + Android SDK/NDK + Rust (Android + WASM targets) for the ADR-015 native PoC.
# Build: scripts/core-build.sh (rebuilds the image when rust-toolchain.toml changes)
# Use:   scripts/core-build.sh (mounts the repo at /workspace)
FROM eclipse-temurin:21-jdk-jammy@sha256:742daacb9316be4b6fc60258f6e4ac2a11882f6c597373c92290a57bfad0d2e0

ENV ANDROID_HOME=/opt/android-sdk
ENV ANDROID_NDK_VERSION=30.0.16248370
ENV ANDROID_NDK_HOME=/opt/android-sdk/ndk/30.0.16248370
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools"

RUN apt-get update \
	&& apt-get install -y --no-install-recommends unzip wget curl ca-certificates build-essential git \
	&& rm -rf /var/lib/apt/lists/*

ARG ANDROID_CLI_TOOLS_URL=https://dl.google.com/android/repository/commandlinetools-linux-16111833_latest.zip
RUN mkdir -p "${ANDROID_HOME}/cmdline-tools" \
	&& wget -q -O /tmp/cmdline-tools.zip "${ANDROID_CLI_TOOLS_URL}" \
	&& unzip -q /tmp/cmdline-tools.zip -d "${ANDROID_HOME}/cmdline-tools" \
	&& mv "${ANDROID_HOME}/cmdline-tools/cmdline-tools" "${ANDROID_HOME}/cmdline-tools/latest" \
	&& rm -f /tmp/cmdline-tools.zip

RUN yes | sdkmanager --licenses >/dev/null \
	&& sdkmanager --install \
		"platform-tools" \
		"platforms;android-37.0" \
		"build-tools;36.0.0" \
		"ndk;${ANDROID_NDK_VERSION}"

# The toolchain comes from the repo's rust-toolchain.toml (named build context "repo", scripts/core-build.sh
# passes --build-context repo=<repo root>).
COPY --from=repo rust-toolchain.toml /tmp/rust-toolchain/rust-toolchain.toml
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none \
	&& cd /tmp/rust-toolchain \
	&& rustup toolchain install \
	&& rustup default "$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)" \
	&& cargo install --locked cargo-ndk@4.1.2 \
	&& chmod -R a+rwX /opt/cargo /opt/rustup

WORKDIR /workspace

# sccache for local builds (scripts/lib/build-image.sh turns it on; CI and determinism/release builds run without it).
# Release page: https://github.com/mozilla/sccache/releases ; the checksums are the release's .sha256 assets.
# Renovate bumps SCCACHE_VERSION only: update both checksums in the same PR.
ARG TARGETARCH
ARG SCCACHE_VERSION=0.18.0
ARG SCCACHE_SHA256_X86_64=45f1447fbe231e3037bde351ef70677dd212216c8d62ae7ca409fecc4d6acc89
ARG SCCACHE_SHA256_AARCH64=2b3284d5da3b46a47dc4229e75bb7b88ac4aa99c8d754fb7d2f84997e5a4354a
RUN case "${TARGETARCH}" in \
		amd64) arch=x86_64; sum="${SCCACHE_SHA256_X86_64}" ;; \
		arm64) arch=aarch64; sum="${SCCACHE_SHA256_AARCH64}" ;; \
		*) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
	esac \
	&& name="sccache-v${SCCACHE_VERSION}-${arch}-unknown-linux-musl" \
	&& curl -fsSL -o "/tmp/${name}.tar.gz" "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/${name}.tar.gz" \
	&& echo "${sum}  /tmp/${name}.tar.gz" | sha256sum -c - \
	&& tar -xzf "/tmp/${name}.tar.gz" -C /tmp \
	&& install -m 0755 "/tmp/${name}/sccache" /usr/local/bin/sccache \
	&& rm -rf "/tmp/${name}" "/tmp/${name}.tar.gz"
