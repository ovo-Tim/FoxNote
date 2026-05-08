import { createElement } from "react";
import { createRoot } from "react-dom/client";
import { loadNoteAttachment, loadNoteImageAttachment } from "./noteApi";
import {
  renderTypstToPngWithTheme,
  renderTypstToPortableSvgWithTheme,
} from "./typstPreview";
import { TldrawImage } from "tldraw";

const DEBUG_BLOCK_EXPORT = true;

function sanitizeSvgMarkup(svg: string): string {
  return svg.replace(/&(?!(?:#\d+|#x[0-9a-fA-F]+|[a-zA-Z][\w.-]*);)/g, "&amp;");
}

function escapeXml(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function decodePngSize(bytes: Uint8Array): { width: number; height: number } {
  if (bytes.length < 24) {
    return { width: 0, height: 0 };
  }

  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  for (let i = 0; i < signature.length; i += 1) {
    if (bytes[i] !== signature[i]) {
      return { width: 0, height: 0 };
    }
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = view.getUint32(16);
  const height = view.getUint32(20);
  return { width, height };
}

function toBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const chunk = bytes.subarray(offset, offset + chunkSize);
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

export async function rasterizeSvgToPng(svg: string): Promise<{
  bytes: Uint8Array;
  width: number;
  height: number;
}> {
  const parseDimension = (value: string | null | undefined): number => {
    if (!value) {
      return 0;
    }
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
  };

  const extractSvgDimensions = (source: string): { width: number; height: number } => {
    const widthMatch = source.match(/<svg\b[^>]*\bwidth\s*=\s*['"]([^'"]+)['"]/i);
    const heightMatch = source.match(/<svg\b[^>]*\bheight\s*=\s*['"]([^'"]+)['"]/i);
    const viewBoxMatch = source.match(/<svg\b[^>]*\bviewBox\s*=\s*['"]([^'"]+)['"]/i);

    const widthRaw = parseDimension(widthMatch?.[1]);
    const heightRaw = parseDimension(heightMatch?.[1]);

    let widthFromViewBox = 0;
    let heightFromViewBox = 0;
    if (viewBoxMatch?.[1]) {
      const parts = viewBoxMatch[1]
        .split(/[\s,]+/)
        .map((part) => Number.parseFloat(part))
        .filter((part) => Number.isFinite(part));
      if (parts.length === 4) {
        widthFromViewBox = parts[2] > 0 ? parts[2] : 0;
        heightFromViewBox = parts[3] > 0 ? parts[3] : 0;
      }
    }

    return {
      width: Math.max(1, Math.round(widthRaw || widthFromViewBox || 1200)),
      height: Math.max(1, Math.round(heightRaw || heightFromViewBox || 800)),
    };
  };

  const ensureSvgNamespace = (source: string): string => {
    if (!/<svg\b/i.test(source)) {
      return source;
    }
    if (/xmlns\s*=\s*['"]http:\/\/www\.w3\.org\/2000\/svg['"]/i.test(source)) {
      return source;
    }
    return source.replace(/<svg\b/i, '<svg xmlns="http://www.w3.org/2000/svg"');
  };

  const sanitizedSvg = ensureSvgNamespace(sanitizeSvgMarkup(svg));
  const rawSvg = ensureSvgNamespace(svg);
  const candidates = rawSvg === sanitizedSvg ? [rawSvg] : [rawSvg, sanitizedSvg];

  if (DEBUG_BLOCK_EXPORT) {
    console.debug("[BlockExport] rasterize start", {
      length: svg.length,
      sanitizedLength: sanitizedSvg.length,
      hasSvgTag: /<svg[\s>]/i.test(svg),
      hasXmlns: /xmlns\s*=/.test(svg),
      preview: svg.slice(0, 220).replace(/\s+/g, " "),
    });
  }

  let lastError: unknown = null;

  for (const candidate of candidates) {
    const requestedSize = extractSvgDimensions(candidate);
    const image = new Image();
    image.decoding = "async";
    const svgBlob = new Blob([candidate], {
      type: "image/svg+xml;charset=utf-8",
    });
    const blobUrl = URL.createObjectURL(svgBlob);

    try {
      await new Promise<void>((resolve, reject) => {
        image.onload = () => resolve();
        image.onerror = (event) => reject(event);
        image.src = blobUrl;
      });

      const width = Math.max(1, Math.round(image.naturalWidth || requestedSize.width));
      const height = Math.max(1, Math.round(image.naturalHeight || requestedSize.height));

      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;

      const ctx = canvas.getContext("2d");
      if (!ctx) {
        throw new Error("Failed to create canvas context");
      }

      ctx.drawImage(image, 0, 0, width, height);

      const blob = await new Promise<Blob | null>((resolve) => {
        canvas.toBlob((value) => resolve(value), "image/png");
      });

      if (!blob) {
        throw new Error("Failed to encode PNG");
      }

      const bytes = new Uint8Array(await blob.arrayBuffer());
      if (DEBUG_BLOCK_EXPORT) {
        console.debug("[BlockExport] rasterize success", {
          width,
          height,
          pngBytes: bytes.length,
          usedSanitizedCandidate: candidate === sanitizedSvg,
        });
      }
      return { bytes, width, height };
    } catch (error) {
      lastError = error;
      console.error("[BlockExport] failed to decode SVG candidate", {
        width: requestedSize.width,
        height: requestedSize.height,
        usedSanitizedCandidate: candidate === sanitizedSvg,
        preview: candidate.slice(0, 500),
      });
    } finally {
      URL.revokeObjectURL(blobUrl);
    }
  }

  throw new Error(`Failed to decode SVG image${lastError ? `: ${String(lastError)}` : ""}`);
}

export function svgTextBlock(
  text: string,
  options?: {
    width?: number;
    height?: number;
    fontSize?: number;
    fontWeight?: number;
    background?: string;
    color?: string;
  },
): string {
  const width = Math.max(120, options?.width ?? 960);
  const height = Math.max(60, options?.height ?? 120);
  const fontSize = Math.max(12, options?.fontSize ?? 34);
  const fontWeight = options?.fontWeight ?? 600;
  const background = options?.background ?? "transparent";
  const color = options?.color ?? "#18202c";
  const escaped = escapeXml(text.trim() || "Untitled");

  return [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">`,
    `<rect width="${width}" height="${height}" fill="${background}" />`,
    `<text x="20" y="${Math.max(32, fontSize + 8)}" font-size="${fontSize}" font-weight="${fontWeight}" fill="${color}" font-family="ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif">${escaped}</text>`,
    "</svg>",
  ].join("");
}

export async function exportTextAsGraphic(
  text: string,
  format: "svg" | "png",
  options?: {
    width?: number;
    height?: number;
    fontSize?: number;
    fontWeight?: number;
    background?: string;
    color?: string;
  },
): Promise<{ bytes: Uint8Array; width: number; height: number; mimeType: string }> {
  const svg = svgTextBlock(text, options);
  const width = Math.max(120, options?.width ?? 960);
  const height = Math.max(60, options?.height ?? 120);

  if (format === "svg") {
    return {
      bytes: new TextEncoder().encode(svg),
      width,
      height,
      mimeType: "image/svg+xml",
    };
  }

  const png = await rasterizeSvgToPng(svg);
  return {
    bytes: png.bytes,
    width: png.width,
    height: png.height,
    mimeType: "image/png",
  };
}

export async function exportTypst(
  source: string,
  format: "svg" | "png",
): Promise<{ bytes: Uint8Array; width: number; height: number; mimeType: string }> {
  if (format === "png") {
    const png = await renderTypstToPngWithTheme(source, { darkMode: false });
    return { ...png, mimeType: "image/png" };
  }

  const svg = await renderTypstToPortableSvgWithTheme(source, { darkMode: false });

  const bytes = new TextEncoder().encode(svg);
  const parsed = new DOMParser().parseFromString(svg, "image/svg+xml");
  const root = parsed.documentElement;
  const width = Math.max(1, Math.round(Number.parseFloat(root.getAttribute("width") ?? "1200") || 1200));
  const height = Math.max(1, Math.round(Number.parseFloat(root.getAttribute("height") ?? "800") || 800));
  return { bytes, width, height, mimeType: "image/svg+xml" };
}

export async function exportImageAttachment(
  noteId: string,
  path: string,
  format: "svg" | "png",
): Promise<{ bytes: Uint8Array; width: number; height: number; mimeType: string }> {
  const payload = await loadNoteImageAttachment(noteId, path);
  const bytes = Uint8Array.from(payload.bytes ?? []);
  const size = decodePngSize(bytes);

  if (format === "png") {
    return {
      bytes,
      width: size.width || 1,
      height: size.height || 1,
      mimeType: "image/png",
    };
  }

  const href = `data:${payload.mimeType || "image/png"};base64,${toBase64(bytes)}`;
  const width = size.width || 800;
  const height = size.height || 600;
  const svg = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">`,
    `<image href="${href}" width="${width}" height="${height}" preserveAspectRatio="xMidYMid meet" />`,
    "</svg>",
  ].join("");

  return {
    bytes: new TextEncoder().encode(svg),
    width,
    height,
    mimeType: "image/svg+xml",
  };
}

function renderTldrawImage(
  snapshot: unknown,
  format: "svg" | "png",
): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    if (DEBUG_BLOCK_EXPORT) {
      console.debug("[BlockExport] tldraw render start", {
        format,
        snapshotType: typeof snapshot,
        hasSnapshot: snapshot != null,
      });
    }

    const host = document.createElement("div");
    host.style.position = "fixed";
    host.style.left = "-10000px";
    host.style.top = "-10000px";
    host.style.width = "1200px";
    host.style.height = "900px";
    document.body.appendChild(host);

    const root = createRoot(host);
    let done = false;
    let sawImg = false;
    let resolveInFlight = false;
    const startedAt = Date.now();
    let pollId: number | null = null;
    let timeoutId: number | null = null;
    let nextProgressLogMs = 1000;

    const observer = new MutationObserver(() => {
      void tryResolveFromImg();
    });

    const finish = (fn: () => void) => {
      if (done) {
        return;
      }
      done = true;
      try {
        fn();
      } finally {
        if (pollId != null) {
          window.clearInterval(pollId);
        }
        if (timeoutId != null) {
          window.clearTimeout(timeoutId);
        }
        observer.disconnect();
        root.unmount();
        host.remove();
      }
    };

    const tryResolveFromImg = async () => {
      if (done || resolveInFlight) {
        return;
      }

      const img = host.querySelector("img");
      if (!img) {
        return;
      }

      if (!sawImg) {
        sawImg = true;
        if (DEBUG_BLOCK_EXPORT) {
          console.debug("[BlockExport] tldraw image element detected", {
            elapsedMs: Date.now() - startedAt,
            hasSrc: Boolean(img.getAttribute("src")),
            complete: img.complete,
            naturalWidth: img.naturalWidth,
            naturalHeight: img.naturalHeight,
          });
        }
      }

      const src = img.getAttribute("src") ?? "";
      const elapsedMs = Date.now() - startedAt;
      if (DEBUG_BLOCK_EXPORT && elapsedMs >= nextProgressLogMs) {
        nextProgressLogMs += 1000;
        console.debug("[BlockExport] tldraw waiting for image readiness", {
          elapsedMs,
          hasSrc: Boolean(src),
          complete: img.complete,
          naturalWidth: img.naturalWidth,
          naturalHeight: img.naturalHeight,
        });
      }

      if (!src || !img.complete) {
        return;
      }

      resolveInFlight = true;
      try {
        const response = await fetch(src);
        if (!response.ok) {
          throw new Error(`Failed to fetch tldraw blob URL: ${response.status}`);
        }
        const blob = await response.blob();
        const bytes = new Uint8Array(await blob.arrayBuffer());

        if (DEBUG_BLOCK_EXPORT) {
          console.debug("[BlockExport] tldraw render success", {
            elapsedMs: Date.now() - startedAt,
            format,
            blobType: blob.type,
            bytes: bytes.length,
            imageWidth: img.naturalWidth,
            imageHeight: img.naturalHeight,
          });
        }
        finish(() => {
          resolve(bytes);
        });
      } catch (error) {
        finish(() => {
          reject(error);
        });
      } finally {
        resolveInFlight = false;
      }
    };

    observer.observe(host, { childList: true, subtree: true });
    root.render(
      createElement(TldrawImage as any, {
        snapshot,
        format,
        darkMode: false,
        background: false,
        padding: 12,
        scale: 1,
      }),
    );

    pollId = window.setInterval(() => {
      void tryResolveFromImg();
    }, 60);

    timeoutId = window.setTimeout(() => {
      if (done) {
        return;
      }
      finish(() => {
        console.error("[BlockExport] tldraw render timeout", {
          elapsedMs: Date.now() - startedAt,
          format,
          sawImg,
          childCount: host.querySelectorAll("*").length,
          hostPreview: host.innerHTML.slice(0, 500),
        });
        reject(new Error("Timed out while rendering tldraw export"));
      });
    }, 15000);
  });
}

export async function exportCanvasAttachment(
  noteId: string,
  path: string,
  format: "svg" | "png",
): Promise<{ bytes: Uint8Array; width: number; height: number; mimeType: string }> {
  const payload = await loadNoteAttachment(noteId, path, "application/vnd.tldraw+json");
  const bytes = Uint8Array.from(payload.bytes ?? []);
  const text = new TextDecoder().decode(bytes);
  const snapshot = JSON.parse(text);

  const rendered = await renderTldrawImage(snapshot, format);
  if (format === "svg") {
    const svg = new TextDecoder().decode(rendered);
    const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
    const root = doc.documentElement;
    const width = Math.max(1, Math.round(Number.parseFloat(root.getAttribute("width") ?? "1200") || 1200));
    const height = Math.max(1, Math.round(Number.parseFloat(root.getAttribute("height") ?? "800") || 800));
    return { bytes: rendered, width, height, mimeType: "image/svg+xml" };
  }

  const size = decodePngSize(rendered);
  return {
    bytes: rendered,
    width: size.width || 1,
    height: size.height || 1,
    mimeType: "image/png",
  };
}
