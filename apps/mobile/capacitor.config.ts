import type { CapacitorConfig } from "@capacitor/core";

const config: CapacitorConfig = {
  appId: "org.guardedsleep.app",
  appName: "GuardedSleep",
  webDir: "build",
  server: {
    androidScheme: "https",
  },
};

export default config;
