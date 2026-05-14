export type SessionUiStatus = "idle" | "connecting" | "connected" | "failed";

export function connectionStatusPillI18nKey(status: SessionUiStatus): string {
  switch (status) {
    case "idle":
      return "connectionStatus.sessionIdle";
    case "connecting":
      return "connectionStatus.sessionConnecting";
    case "connected":
      return "connectionStatus.sessionConnected";
    case "failed":
      return "connectionStatus.sessionFailed";
  }
}
