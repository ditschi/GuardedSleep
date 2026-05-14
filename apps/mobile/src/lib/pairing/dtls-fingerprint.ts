/** First `a=fingerprint:sha-256` line in SDP, normalized to lowercase hex without separators. */
export function extractSha256FingerprintFromSdp(sdp: string): string | null {
  const m = sdp.match(/a=fingerprint:sha-256\s+([0-9A-Fa-f:\s]+)/i);
  if (!m?.[1]) return null;
  return m[1].replace(/[\s:]/g, "").toLowerCase();
}
