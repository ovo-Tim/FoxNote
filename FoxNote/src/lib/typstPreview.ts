import { $typst } from "@myriaddreamin/typst.ts";
import compilerWasmUrl from "@myriaddreamin/typst-ts-web-compiler/pkg/typst_ts_web_compiler_bg.wasm?url";
import rendererWasmUrl from "@myriaddreamin/typst-ts-renderer/pkg/typst_ts_renderer_bg.wasm?url";

let initialized = false;

function isAlreadyInitializedError(error: unknown): boolean {
  const message =
    typeof error === "object" && error !== null && "message" in error
      ? String((error as { message?: unknown }).message ?? "")
      : String(error ?? "");

  return /initialized/i.test(message);
}

interface TypstRenderOptions {
  darkMode?: boolean;
  pageWidth?: string;
  pageHeight?: string;
}

function buildThemedSource(source: string, options: TypstRenderOptions = {}): string {
  const darkMode = options.darkMode ?? false;
  const pageWidth = options.pageWidth ?? "auto";
  const pageHeight = options.pageHeight ?? "auto";
  const textColor = darkMode ? "#edf2ff" : "#111111";

  return [
    `#set page(width: ${pageWidth}, height: ${pageHeight}, margin: 0.5pt)`,
    `#set text(size: 20pt, fill: rgb("${textColor}"))`,
    `#show math.equation: set text(top-edge: "bounds", bottom-edge: "bounds")`,
    "",
    source,
  ].join("\n");
}

function ensureInitialized() {
  if (initialized) {
    return;
  }

  try {
    $typst.setCompilerInitOptions({
      getModule: () => compilerWasmUrl,
    });
  } catch (error) {
    if (!isAlreadyInitializedError(error)) {
      throw error;
    }
  }

  try {
    $typst.setRendererInitOptions({
      getModule: () => rendererWasmUrl,
    });
  } catch (error) {
    if (!isAlreadyInitializedError(error)) {
      throw error;
    }
  }

  initialized = true;
}

function renderSvg(mainContent: string, portable: boolean): Promise<string> {
  if (!portable) {
    return $typst.svg({ mainContent });
  }

  return $typst.svg({
    mainContent,
    data_selection: { js: false },
  } as any);
}

export async function renderTypstToSvg(source: string): Promise<string> {
  ensureInitialized();

  return renderSvg(source, false);
}

export async function renderTypstToSvgWithTheme(
  source: string,
  options: TypstRenderOptions = {},
): Promise<string> {
  ensureInitialized();

  const themedSource = buildThemedSource(source, options);

  return renderSvg(themedSource, false);
}

export async function renderTypstToPortableSvgWithTheme(
  source: string,
  options: TypstRenderOptions = {},
): Promise<string> {
  ensureInitialized();

  const themedSource = buildThemedSource(source, options);

  return renderSvg(themedSource, true);
}

export async function renderTypstToPngWithTheme(
  source: string,
  options: TypstRenderOptions = {},
): Promise<{ bytes: Uint8Array; width: number; height: number }> {
  ensureInitialized();

  const themedSource = buildThemedSource(source, options);
  const mount = document.createElement("div");
  mount.style.position = "fixed";
  mount.style.left = "-10000px";
  mount.style.top = "-10000px";
  mount.style.width = "1200px";
  mount.style.height = "1200px";
  mount.style.pointerEvents = "none";
  document.body.appendChild(mount);

  try {
    await $typst.canvas(mount, { mainContent: themedSource });
    await Promise.resolve();

    const canvases = Array.from(mount.querySelectorAll("canvas"));
    const canvas =
      canvases.find((item) => item.width > 0 && item.height > 0) ??
      canvases[0] ??
      null;

    if (!canvas) {
      throw new Error("Failed to render Typst canvas");
    }

    const blob = await new Promise<Blob | null>((resolve) => {
      canvas.toBlob((value) => resolve(value), "image/png");
    });

    if (!blob) {
      throw new Error("Failed to encode Typst PNG");
    }

    const bytes = new Uint8Array(await blob.arrayBuffer());
    return {
      bytes,
      width: Math.max(1, canvas.width || 1),
      height: Math.max(1, canvas.height || 1),
    };
  } finally {
    mount.remove();
  }
}
