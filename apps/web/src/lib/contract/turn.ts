/** Outcome of saving the TURN entry (mirrors the Android `TurnServerInput` result). */
export type TurnSaveResult = "saved" | "cleared" | "invalid_address" | "invalid_credentials";

export const TURN_CREDENTIAL_MAX = 256;

// `turn:` or `turns:`, a host (IPv4 and DNS names share the label charset; bracketed IPv6), an
// optional port and an optional transport. Nothing else: no whitespace, path or userinfo.
const TURN_ADDRESS =
  /^turns?:(?:\[[0-9A-Fa-f:.]+\]|[A-Za-z0-9][A-Za-z0-9.-]*)(?::(\d{1,5}))?(?:\?transport=(?:udp|tcp))?$/;

export function isTurnAddress(text: string): boolean {
  const match = TURN_ADDRESS.exec(text.trim());
  if (match === null) return false;
  const port = match[1];
  if (port === undefined) return true;
  const n = Number(port);
  return n >= 1 && n <= 65535;
}

/**
 * Pure check of a TURN entry before it is saved. User name and password are both set or both
 * empty, each at most 256 characters.
 */
export function validateTurnServer(
  url: string,
  username: string,
  password: string,
): "valid" | "invalid_address" | "invalid_credentials" {
  if (!isTurnAddress(url)) return "invalid_address";
  if ((username === "") !== (password === "")) return "invalid_credentials";
  if (username.length > TURN_CREDENTIAL_MAX || password.length > TURN_CREDENTIAL_MAX) {
    return "invalid_credentials";
  }
  return "valid";
}

/** The TURN entry as the platform keeps it. The password never reaches a store. */
export interface TurnEntry {
  url: string;
  username: string;
  password: string;
}

/** What the save does with an empty password field: keep the saved one for an unchanged entry. */
export function resolveTurnSave(
  saved: TurnEntry | null,
  url: string,
  username: string,
  password: string,
): { result: TurnSaveResult; entry: TurnEntry | null } {
  const trimmed = url.trim();
  if (trimmed === "") return { result: "cleared", entry: null };
  const keep =
    password === "" && saved !== null && saved.url === trimmed && saved.username === username;
  const effective = keep ? saved.password : password;
  const check = validateTurnServer(trimmed, username, effective);
  if (check !== "valid") return { result: check, entry: saved };
  return { result: "saved", entry: { url: trimmed, username, password: effective } };
}

/** Settings fields derived from the entry; the password is reduced to a flag. */
export function turnSettings(entry: TurnEntry | null): {
  turnUrl: string | null;
  turnUsername: string | null;
  turnPasswordSaved: boolean;
} {
  return {
    turnUrl: entry?.url ?? null,
    turnUsername: entry !== null && entry.username !== "" ? entry.username : null,
    turnPasswordSaved: entry !== null && entry.password !== "",
  };
}
