import QRCode from "qrcode";

/** Quiet zone in modules. The code is shown light on dark, where scanners need it generous. */
export const QR_MARGIN = 4;

export interface QrShape {
  /** Modules per side including the quiet zone (the SVG `viewBox` is `0 0 size size`). */
  size: number;
  /** One path of horizontal runs for every dark module of the standard code. */
  path: string;
}

/**
 * The QR code of `text` as an SVG path, so the screen renders it without `innerHTML` (the page
 * runs under Trusted Types) and without a canvas. Error correction level M, no logo
 * (rebuild-web-client design.md). The path draws the modules that are dark in the standard
 * code; the screen paints them light on a dark ground ("inverted") at reduced brightness.
 */
export function qrShape(text: string): QrShape {
  const { modules } = QRCode.create(text, { errorCorrectionLevel: "M" });
  const n = modules.size;
  const parts: string[] = [];
  for (let y = 0; y < n; y++) {
    let x = 0;
    while (x < n) {
      if (!modules.get(y, x)) {
        x++;
        continue;
      }
      const start = x;
      while (x < n && modules.get(y, x)) x++;
      parts.push(`M${start + QR_MARGIN} ${y + QR_MARGIN}h${x - start}v1h${start - x}z`);
    }
  }
  return { size: n + 2 * QR_MARGIN, path: parts.join("") };
}
