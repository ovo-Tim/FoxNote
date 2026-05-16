import { $typst } from "@myriaddreamin/typst.ts";
import compilerWasmUrl from "@myriaddreamin/typst-ts-web-compiler/pkg/typst_ts_web_compiler_bg.wasm?url";
import rendererWasmUrl from "@myriaddreamin/typst-ts-renderer/pkg/typst_ts_renderer_bg.wasm?url";
import { TypstSnippet } from "@myriaddreamin/typst.ts/dist/esm/contrib/snippet.mjs";
import { loadFonts } from "@myriaddreamin/typst.ts/dist/esm/options.init.mjs";
import sourceHanSerifRegularUrl from "@fontpkg/source-han-serif-sc/SourceHanSerifSC-Regular.otf?url";
import sourceHanSerifBoldUrl from "@fontpkg/source-han-serif-sc/SourceHanSerifSC-Bold.otf?url";

interface TypstRenderOptions {
  darkMode?: boolean;
  pageWidth?: string;
  pageHeight?: string;
}

interface RenderRequest {
  type: "render-svg";
  requestId: number;
  source: string;
  options?: TypstRenderOptions;
  portable?: boolean;
}

interface RenderSuccessResponse {
  type: "render-svg-result";
  requestId: number;
  ok: true;
  svg: string;
}

interface RenderErrorResponse {
  type: "render-svg-result";
  requestId: number;
  ok: false;
  error: string;
}

type RenderResponse = RenderSuccessResponse | RenderErrorResponse;

let initialized = false;
let initPromise: Promise<void> | null = null;

function isAlreadyInitializedError(error: unknown): boolean {
  const message =
    typeof error === "object" && error !== null && "message" in error
      ? String((error as { message?: unknown }).message ?? "")
      : String(error ?? "");

  return /initialized/i.test(message);
}

function buildThemedSource(source: string, options: TypstRenderOptions = {}): string {
  const darkMode = options.darkMode ?? false;
  const pageWidth = options.pageWidth ?? "auto";
  const pageHeight = options.pageHeight ?? "auto";
  const textColor = darkMode ? "#edf2ff" : "#111111";

  return [
    '#import "@preview/note-me:0.6.0": *',
    '#import "@preview/mitex:0.2.7": *',
    `#set page(width: ${pageWidth}, height: ${pageHeight}, margin: 0.5pt)`,
    `#set text(size: 18pt, fill: rgb("${textColor}"), font: ("Source Han Serif SC", "Noto Serif CJK SC", "Libertinus Serif", "New Computer Modern", "DejaVu Sans Mono"))`,
    `#set text(top-edge: "bounds", bottom-edge: "bounds")`,
    `#show math.equation: set text(top-edge: "bounds", bottom-edge: "bounds")`,
    `#show link: set text(fill: aqua)`,
    `#show link: underline`,
    "",
    source,
  ].join("\n");
}

async function ensureInitialized() {
  if (initialized) {
    return;
  }

  if (initPromise) {
    await initPromise;
    return;
  }

  initPromise = (async () => {
    try {
      $typst.setCompilerInitOptions({
        getModule: () => compilerWasmUrl,
        beforeBuild: [loadFonts([sourceHanSerifRegularUrl, sourceHanSerifBoldUrl], { assets: ["text"] })],
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

    try {
      $typst.use(await TypstSnippet.fetchPackageRegistry());
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error ?? "");
      if (!/already prepare uses|already.*use|initialized/i.test(message)) {
        throw error;
      }
    }

    initialized = true;
  })();

  try {
    await initPromise;
  } finally {
    initPromise = null;
  }
}

function renderSvg(mainContent: string, portable: boolean): Promise<string> {
  if (!portable) {
    return $typst.svg({ mainContent });
  }

  return $typst.svg({
    mainContent,
    data_selection: { body: true, defs: true, css: true, js: false },
  });
}

async function handleRenderRequest(request: RenderRequest): Promise<RenderResponse> {
  try {
    await ensureInitialized();
    const themedSource = buildThemedSource(request.source, request.options ?? {});
    const svg = await renderSvg(themedSource, Boolean(request.portable));
    return {
      type: "render-svg-result",
      requestId: request.requestId,
      ok: true,
      svg,
    };
  } catch (error) {
    return {
      type: "render-svg-result",
      requestId: request.requestId,
      ok: false,
      error: error instanceof Error ? error.message : String(error ?? "Unknown render error"),
    };
  }
}

self.addEventListener("message", (event: MessageEvent<RenderRequest>) => {
  const request = event.data;
  if (!request || request.type !== "render-svg") {
    return;
  }

  void handleRenderRequest(request).then((response) => {
    self.postMessage(response);
  });
});
