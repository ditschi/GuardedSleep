# Dev: Node 22 + Temurin JDK 21 + Android SDK (Capacitor / Gradle) in one image.
# JDK 21 matches Capacitor/Android `source release 21`. Bookworm often has no OpenJDK 21 in default APT,
# so we copy Temurin from the official image (same family as docker/android-build).
# Build context: .devcontainer/
FROM eclipse-temurin:21-jdk-jammy AS temurin21

FROM mcr.microsoft.com/devcontainers/typescript-node:1-22-bookworm

USER root

COPY --from=temurin21 /opt/java/openjdk /opt/java/openjdk

ENV JAVA_HOME=/opt/java/openjdk
ENV ANDROID_HOME=/opt/android-sdk
ARG ANDROID_CLI_TOOLS_URL=https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
	&& apt-get install -y --no-install-recommends \
		python3 \
		python3-pip \
		gh \
		unzip \
		wget \
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
	&& chown -R node:node "${ANDROID_HOME}" \
	&& apt-get clean \
	&& rm -rf /var/lib/apt/lists/*

# System-wide so `git commit` works when the compose user is root (rootless uid map);
# post-create still installs to ~/.local for the node user, but hooks must find pre-commit on PATH.
RUN python3 -m pip install --no-cache-dir --break-system-packages pre-commit 2>/dev/null \
	|| python3 -m pip install --no-cache-dir pre-commit

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
	'PATH="${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${PATH}"' \
	'export PATH' \
	> /etc/profile.d/guardedsleep-android.sh \
	&& chmod 644 /etc/profile.d/guardedsleep-android.sh

RUN printf '\n# GuardedSleep Android (after nvm in .bashrc)\nexport JAVA_HOME=/opt/java/openjdk\nexport ANDROID_HOME=/opt/android-sdk\nexport PATH="${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${PATH}"\n' >> /home/node/.bashrc \
	&& chown node:node /home/node/.bashrc

USER node
WORKDIR /workspace
