import { describe, expect, it } from "vitest";
import { readRemoteDtlsFingerprint } from "./dtls-stats.js";

function pcWith(entries: Record<string, Record<string, unknown>>) {
  const map = new Map(Object.entries(entries));
  return { getStats: async () => map };
}

describe("readRemoteDtlsFingerprint", () => {
  it("follows transport.remoteCertificateId to the certificate fingerprint", async () => {
    const pc = pcWith({
      T1: {
        type: "transport",
        dtlsState: "connected",
        remoteCertificateId: "CR",
        localCertificateId: "CL",
      },
      CL: { type: "certificate", fingerprint: "00:11", fingerprintAlgorithm: "sha-256" },
      CR: { type: "certificate", fingerprint: "AB:CD:EF", fingerprintAlgorithm: "sha-256" },
    });
    expect(await readRemoteDtlsFingerprint(pc)).toBe("AB:CD:EF");
  });

  it("returns null when the transport stats carry no remote certificate (Firefox)", async () => {
    const pc = pcWith({
      T1: { type: "transport", dtlsState: "connected", localCertificateId: "CL" },
      CL: { type: "certificate", fingerprint: "00:11", fingerprintAlgorithm: "sha-256" },
    });
    expect(await readRemoteDtlsFingerprint(pc)).toBeNull();
  });

  it("accepts an upper-case algorithm name", async () => {
    const pc = pcWith({
      T1: { type: "transport", remoteCertificateId: "CR" },
      CR: { type: "certificate", fingerprint: "AB", fingerprintAlgorithm: "SHA-256" },
    });
    expect(await readRemoteDtlsFingerprint(pc)).toBe("AB");
  });

  it("refuses other algorithms", async () => {
    const pc = pcWith({
      T1: { type: "transport", remoteCertificateId: "CR" },
      CR: { type: "certificate", fingerprint: "AB", fingerprintAlgorithm: "sha-1" },
    });
    expect(await readRemoteDtlsFingerprint(pc)).toBeNull();
  });

  it("returns null while no transport or certificate is listed", async () => {
    expect(await readRemoteDtlsFingerprint(pcWith({}))).toBeNull();
    expect(
      await readRemoteDtlsFingerprint(pcWith({ T1: { type: "transport", dtlsState: "new" } })),
    ).toBeNull();
    expect(
      await readRemoteDtlsFingerprint(
        pcWith({ T1: { type: "transport", remoteCertificateId: "X" } }),
      ),
    ).toBeNull();
  });
});
