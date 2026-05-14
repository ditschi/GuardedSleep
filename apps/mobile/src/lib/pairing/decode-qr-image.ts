/**
 * Decode a QR code from an image file using ZXing, with fallbacks for large or
 * awkward camera photos (downscale + redraw) and finally all barcode formats.
 */
export async function decodeQrFromImageFile(file: File): Promise<string> {
  const { BrowserMultiFormatReader, BarcodeFormat } = await import("@zxing/browser");
  const url = URL.createObjectURL(file);

  const attempts: (() => Promise<string>)[] = [
    async () => {
      const reader = new BrowserMultiFormatReader();
      reader.possibleFormats = [BarcodeFormat.QR_CODE];
      return (await reader.decodeFromImageUrl(url)).getText();
    },
    async () => {
      const reader = new BrowserMultiFormatReader();
      reader.possibleFormats = [BarcodeFormat.QR_CODE];
      const bmp = await createImageBitmap(file);
      try {
        const canvas = document.createElement("canvas");
        const max = 2048;
        let w = bmp.width;
        let h = bmp.height;
        if (w > max || h > max) {
          const s = max / Math.max(w, h);
          w = Math.round(w * s);
          h = Math.round(h * s);
        }
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext("2d");
        if (!ctx) throw new Error("no 2d context");
        ctx.drawImage(bmp, 0, 0, w, h);
        return reader.decodeFromCanvas(canvas).getText();
      } finally {
        bmp.close();
      }
    },
    async () => {
      const reader = new BrowserMultiFormatReader();
      return (await reader.decodeFromImageUrl(url)).getText();
    },
  ];

  let lastErr: unknown;
  try {
    for (const fn of attempts) {
      try {
        return await fn();
      } catch (e) {
        lastErr = e;
      }
    }
  } finally {
    URL.revokeObjectURL(url);
  }

  throw lastErr instanceof Error ? lastErr : new Error("QR decode failed");
}
