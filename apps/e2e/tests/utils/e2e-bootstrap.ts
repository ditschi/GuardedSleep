/** Seeds localStorage before app boot so E2E skips onboarding and uses default prefs. */
export const E2E_APP_PREFERENCES_JSON = JSON.stringify({
  onboardingCompleted: true,
  showAdvancedSettings: false,
  showConnectionDetails: false,
  nightMode: "system",
  wakeLockInListenMode: true,
  listenHintSeen: true,
  alertAudioOutput: "default",
  alertPlaybackGain: 1,
  alertMinGainDuringAlert: false,
  heartbeatIntervalSec: 10,
  probeRoutingHintSeen: true,
});
