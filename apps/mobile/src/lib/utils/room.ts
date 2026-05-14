import { PASSPHRASE_WORDS } from "./passphrase-words.js";

const ROOM_ID_CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
const DEFAULT_GENERATED_LENGTH = 12;
const PASSPHRASE_WORD_COUNT = 4;
const ROOM_ID_MAX_LEN = 64;

function randomUint32(): number {
  const a = new Uint32Array(1);
  crypto.getRandomValues(a);
  return a[0]!;
}

/**
 * Human-readable room id: random words joined by `-`, valid for `normalizeRoomId`.
 * Length is kept within 4–64 characters by reducing word count if needed.
 */
export function generatePassphraseRoomId(): string {
  const n = PASSPHRASE_WORDS.length;
  if (n === 0) return generateRoomId();

  let wordCount = PASSPHRASE_WORD_COUNT;
  while (wordCount >= 2) {
    const words: string[] = [];
    for (let i = 0; i < wordCount; i++) {
      words.push(PASSPHRASE_WORDS[randomUint32() % n]!);
    }
    const s = words.join("-");
    if (s.length <= ROOM_ID_MAX_LEN && s.length >= 4) {
      const norm = normalizeRoomId(s);
      if (norm) return norm;
    }
    wordCount -= 1;
  }
  return generateRoomId();
}

/** Cryptographically random room id (alphanumeric + _-), valid for `normalizeRoomId`. */
export function generateRoomId(length: number = DEFAULT_GENERATED_LENGTH): string {
  const n = Math.max(4, Math.min(64, Math.floor(length)));
  const bytes = new Uint8Array(n);
  crypto.getRandomValues(bytes);
  let s = "";
  for (let i = 0; i < n; i++) {
    s += ROOM_ID_CHARS[bytes[i]! % ROOM_ID_CHARS.length];
  }
  return s;
}

/** Normalizes a user-entered room id for signaling (alphanumeric + _-). */
export function normalizeRoomId(input: string): string | undefined {
  const t = input.trim();
  if (!/^[a-zA-Z0-9_-]{4,64}$/.test(t)) return undefined;
  return t;
}
