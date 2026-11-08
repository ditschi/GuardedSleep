import type { IceServer } from "@guardedsleep/core-wasm";

/** The core's entry as the browser takes it: the three fields, nothing added or dropped. */
export function toRtcIceServer(server: IceServer): RTCIceServer {
  const entry: RTCIceServer = { urls: [...server.urls] };
  if (server.username !== undefined) entry.username = server.username;
  if (server.credential !== undefined) entry.credential = server.credential;
  return entry;
}
