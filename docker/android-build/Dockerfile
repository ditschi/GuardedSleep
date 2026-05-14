# JDK + Android SDK for headless Gradle APK builds (no Android Studio).
# Capacitor 7 / AGP may require Java 21 (source release 21).
# Use after: pnpm --filter @guardedsleep/mobile build && pnpm --filter @guardedsleep/mobile exec cap sync
# Build: docker compose -f docker/android-build/compose.yaml build
FROM eclipse-temurin:21-jdk-jammy

ENV ANDROID_HOME=/opt/android-sdk
ENV PATH="${PATH}:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools"

RUN apt-get update \
	&& apt-get install -y --no-install-recommends unzip wget ca-certificates \
	&& rm -rf /var/lib/apt/lists/*

# Google command-line tools (version pin; bump when updating SDK workflow)
ARG ANDROID_CLI_TOOLS_URL=https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
RUN mkdir -p "${ANDROID_HOME}/cmdline-tools" \
	&& wget -q -O /tmp/cmdline-tools.zip "${ANDROID_CLI_TOOLS_URL}" \
	&& unzip -q /tmp/cmdline-tools.zip -d "${ANDROID_HOME}/cmdline-tools" \
	&& mv "${ANDROID_HOME}/cmdline-tools/cmdline-tools" "${ANDROID_HOME}/cmdline-tools/latest" \
	&& rm -f /tmp/cmdline-tools.zip

# Match apps/mobile/android/variables.gradle (compileSdk 35)
RUN yes | sdkmanager --licenses >/dev/null \
	&& sdkmanager --install \
		"platform-tools" \
		"platforms;android-35" \
		"build-tools;35.0.0"

WORKDIR /workspace
