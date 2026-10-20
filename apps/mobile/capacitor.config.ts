import type { CapacitorConfig } from "@capacitor/core";

const devServer = process.env.CAPACITOR_DEV_SERVER?.trim();

const config: CapacitorConfig = {
  appId: "org.guardedsleep.app",
  appName: "GuardedSleep",
  webDir: "build",
  server: devServer
    ? {
        url: devServer,
        cleartext: devServer.startsWith("http://"),
        androidScheme: "https",
      }
    : {
        androidScheme: "https",
      },
};

export default config;
