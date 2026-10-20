type StatsEntry = { type?: string; [key: string]: unknown };
type StatsReportLike = {
  forEach(cb: (value: StatsEntry) => void): void;
  get(id: string): StatsEntry | undefined;
};

/**
 * SHA-256 fingerprint of the remote DTLS certificate, from `RTCTransportStats.remoteCertificateId`
 * -> `RTCCertificateStats.fingerprint`. This is the certificate the DTLS handshake really
 * authenticated, which the core compares with the one in the signed SDP. Null while the stats do
 * not list it yet or the algorithm is not SHA-256.
 */
export async function readRemoteDtlsFingerprint(pc: {
  getStats(): Promise<unknown>;
}): Promise<string | null> {
  const report = (await pc.getStats()) as StatsReportLike;
  let remoteCertificateId: string | null = null;
  report.forEach((entry) => {
    if (entry.type !== "transport" || typeof entry.remoteCertificateId !== "string") return;
    if (remoteCertificateId === null || entry.dtlsState === "connected") {
      remoteCertificateId = entry.remoteCertificateId;
    }
  });
  if (remoteCertificateId === null) return null;
  const cert = report.get(remoteCertificateId);
  if (!cert || cert.type !== "certificate") return null;
  const algorithm = cert.fingerprintAlgorithm;
  const fingerprint = cert.fingerprint;
  if (typeof algorithm !== "string" || algorithm.toLowerCase() !== "sha-256") return null;
  return typeof fingerprint === "string" && fingerprint !== "" ? fingerprint : null;
}
