import { describe, expect, it, vi } from "vitest";
import { ScanError } from "../contract";
import { createQrScanner, SCAN_INTERVAL_MS, type ScannerEnv } from "./qr-scanner";
import { FakeTime } from "./test-support";

class FakeTrack {
  stopped = false;
  stop(): void {
    this.stopped = true;
  }
}

function fakeStream(): { stream: MediaStream; track: FakeTrack } {
  const track = new FakeTrack();
  const stream = { getTracks: () => [track] } as unknown as MediaStream;
  return { stream, track };
}

function fakeVideo(): HTMLVideoElement {
  const video = document.createElement("video");
  Object.defineProperty(video, "videoWidth", { value: 800 });
  Object.defineProperty(video, "videoHeight", { value: 600 });
  video.play = () => Promise.resolve();
  return video;
}

/** A canvas whose 2d context records the composite operations. */
function fakeCanvas(): { canvas: HTMLCanvasElement; ops: string[] } {
  const ops: string[] = [];
  const ctx = {
    set globalCompositeOperation(value: string) {
      ops.push(value);
    },
    set fillStyle(_value: string) {},
    drawImage: () => undefined,
    fillRect: () => undefined,
  };
  const canvas = {
    width: 0,
    height: 0,
    getContext: () => ctx,
  } as unknown as HTMLCanvasElement;
  return { canvas, ops };
}

function setup(over: Partial<ScannerEnv> = {}) {
  const time = new FakeTime();
  const { stream, track } = fakeStream();
  const { canvas, ops } = fakeCanvas();
  const getUserMedia = vi.fn().mockResolvedValue(stream);
  const env: ScannerEnv = {
    mediaDevices: { getUserMedia },
    barcodeDetector: undefined,
    loadFallback: () => Promise.resolve(() => null),
    createCanvas: () => canvas,
    timers: time,
    ...over,
  };
  return { time, track, getUserMedia, ops, env, scanner: createQrScanner(env) };
}

/** Lets pending promises (frame decoding) settle. */
const settle = async (): Promise<void> => {
  for (let i = 0; i < 6; i++) await Promise.resolve();
};

describe("camera scanner", () => {
  it("asks for the rear camera only when started and never earlier", async () => {
    const s = setup();
    expect(s.getUserMedia).not.toHaveBeenCalled();
    await s.scanner.start(fakeVideo(), () => undefined);
    expect(s.getUserMedia).toHaveBeenCalledWith({
      video: { facingMode: { ideal: "environment" } },
      audio: false,
    });
  });

  it("reports a code from the native detector once and releases the camera", async () => {
    const detect = vi.fn().mockResolvedValue([{ rawValue: '{"v":3}' }]);
    const s = setup({
      barcodeDetector: class {
        detect = detect;
      },
    });
    const seen: string[] = [];
    await s.scanner.start(fakeVideo(), (text) => seen.push(text));
    s.time.advance(SCAN_INTERVAL_MS);
    await settle();
    expect(seen).toEqual(['{"v":3}']);
    expect(s.track.stopped).toBe(true);
    s.time.advance(5 * SCAN_INTERVAL_MS);
    await settle();
    expect(seen).toHaveLength(1);
  });

  it("tries the inverted frame when the code is light on dark", async () => {
    const detect = vi
      .fn()
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([{ rawValue: "inverted" }]);
    const s = setup({
      barcodeDetector: class {
        detect = detect;
      },
    });
    const seen: string[] = [];
    await s.scanner.start(fakeVideo(), (text) => seen.push(text));
    s.time.advance(SCAN_INTERVAL_MS);
    await settle();
    expect(seen).toEqual(["inverted"]);
    expect(s.ops).toEqual(["source-over", "difference"]);
  });

  it("falls back to the JavaScript decoder where the browser has no detector", async () => {
    const decode = vi.fn().mockReturnValueOnce(null).mockReturnValueOnce("from zxing");
    const s = setup({ loadFallback: () => Promise.resolve(decode) });
    const seen: string[] = [];
    await s.scanner.start(fakeVideo(), (text) => seen.push(text));
    s.time.advance(SCAN_INTERVAL_MS);
    await settle();
    expect(seen).toEqual(["from zxing"]);
  });

  it.each([
    ["NotAllowedError", "denied"],
    ["SecurityError", "denied"],
    ["NotFoundError", "no_camera"],
    ["OverconstrainedError", "no_camera"],
  ])("maps %s to %s", async (name, kind) => {
    const s = setup({
      mediaDevices: {
        getUserMedia: () => Promise.reject(new DOMException("nope", name)),
      },
    });
    await expect(s.scanner.start(fakeVideo(), () => undefined)).rejects.toMatchObject({ kind });
  });

  it("is unsupported without media devices (a page that is not a secure context)", async () => {
    const s = setup({ mediaDevices: undefined });
    await expect(s.scanner.start(fakeVideo(), () => undefined)).rejects.toBeInstanceOf(ScanError);
    await expect(s.scanner.start(fakeVideo(), () => undefined)).rejects.toMatchObject({
      kind: "unsupported",
    });
  });

  it("stops the camera and the frame loop", async () => {
    const detect = vi.fn().mockResolvedValue([]);
    const s = setup({
      barcodeDetector: class {
        detect = detect;
      },
    });
    await s.scanner.start(fakeVideo(), () => undefined);
    s.scanner.stop();
    expect(s.track.stopped).toBe(true);
    s.time.advance(3 * SCAN_INTERVAL_MS);
    await settle();
    expect(detect).not.toHaveBeenCalled();
  });

  it("releases a camera that arrives after stop", async () => {
    let grant: (stream: MediaStream) => void = () => undefined;
    const { stream, track } = fakeStream();
    const s = setup({
      mediaDevices: {
        getUserMedia: () =>
          new Promise<MediaStream>((resolve) => {
            grant = resolve;
          }),
      },
    });
    const started = s.scanner.start(fakeVideo(), () => undefined);
    s.scanner.stop();
    grant(stream);
    await started;
    expect(track.stopped).toBe(true);
  });
});
