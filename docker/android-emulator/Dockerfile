# Headless Android emulator for local hybrid E2E (dev-only, requires KVM on Linux).
# Build context: docker/android-emulator/
FROM eclipse-temurin:21-jdk-jammy@sha256:742daacb9316be4b6fc60258f6e4ac2a11882f6c597373c92290a57bfad0d2e0

ENV ANDROID_HOME=/opt/android-sdk
ENV ANDROID_AVD_HOME=/opt/android-avd
ENV PATH="${PATH}:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${ANDROID_HOME}/emulator"
ENV DEBIAN_FRONTEND=noninteractive

ARG ANDROID_CLI_TOOLS_URL=https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
ARG TARGETARCH=amd64
ARG EMULATOR_AVD=guardedsleep_e2e

RUN apt-get update \
	&& apt-get install -y --no-install-recommends \
		unzip \
		wget \
		ca-certificates \
		libnss3 \
		libgl1 \
		libpulse0 \
		libx11-6 \
		libxcomposite1 \
		libxdamage1 \
		libxrandr2 \
		libxext6 \
		libxfixes3 \
		libxi6 \
		libxtst6 \
		libdrm2 \
		libgbm1 \
	&& rm -rf /var/lib/apt/lists/*

RUN mkdir -p "${ANDROID_HOME}/cmdline-tools" "${ANDROID_AVD_HOME}" \
	&& wget -q -O /tmp/cmdline-tools.zip "${ANDROID_CLI_TOOLS_URL}" \
	&& unzip -q /tmp/cmdline-tools.zip -d "${ANDROID_HOME}/cmdline-tools" \
	&& mv "${ANDROID_HOME}/cmdline-tools/cmdline-tools" "${ANDROID_HOME}/cmdline-tools/latest" \
	&& rm -f /tmp/cmdline-tools.zip

RUN set -eu \
	&& if [ "${TARGETARCH}" = "arm64" ]; then \
		SYSIMG="system-images;android-35;google_apis;arm64-v8a"; \
	else \
		SYSIMG="system-images;android-35;google_apis;x86_64"; \
	fi \
	&& yes | sdkmanager --licenses >/dev/null \
	&& sdkmanager --install \
		"platform-tools" \
		"emulator" \
		"platforms;android-35" \
		"${SYSIMG}" \
	&& echo "no" | avdmanager create avd \
		-n "${EMULATOR_AVD}" \
		-k "${SYSIMG}" \
		-d "pixel_6" \
		--force \
	&& printf '%s\n' "${SYSIMG}" > /opt/emulator-sysimg.txt

COPY entrypoint.sh /usr/local/bin/emulator-entrypoint.sh
RUN chmod +x /usr/local/bin/emulator-entrypoint.sh

ENV EMULATOR_AVD="${EMULATOR_AVD}"

ENTRYPOINT ["/usr/local/bin/emulator-entrypoint.sh"]
