# JDK 21 + Android SDK/NDK + Rust (Android + WASM targets) for the ADR-015 native PoC.
# Build: docker build -t guardedsleep-android-native-build:local docker/android-native-build
# Use:   scripts/core-build.sh (mounts the repo at /workspace)
FROM eclipse-temurin:21-jdk-jammy

ENV ANDROID_HOME=/opt/android-sdk
ENV ANDROID_NDK_VERSION=30.0.16248370
ENV ANDROID_NDK_HOME=/opt/android-sdk/ndk/30.0.16248370
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools"

RUN apt-get update \
	&& apt-get install -y --no-install-recommends unzip wget curl ca-certificates build-essential \
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

ARG RUST_VERSION=1.99.1
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_VERSION}" \
	&& rustup target add aarch64-linux-android x86_64-linux-android wasm32-unknown-unknown \
	&& rustup component add clippy rustfmt \
	&& cargo install --locked cargo-ndk@4.1.2 \
	&& chmod -R a+rwX /opt/cargo /opt/rustup

WORKDIR /workspace
