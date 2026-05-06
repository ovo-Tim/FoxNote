import { $typst } from "@myriaddreamin/typst.ts";
import compilerWasmUrl from "@myriaddreamin/typst-ts-web-compiler/pkg/typst_ts_web_compiler_bg.wasm?url";
import rendererWasmUrl from "@myriaddreamin/typst-ts-renderer/pkg/typst_ts_renderer_bg.wasm?url";

let initialized = false;

interface TypstRenderOptions {
  darkMode?: boolean;
  pageWidth?: string;
  pageHeight?: string;
}

function ensureInitialized() {
  if (initialized) {
    return;
  }

  $typst.setCompilerInitOptions({
    getModule: () => compilerWasmUrl,
  });
  $typst.setRendererInitOptions({
    getModule: () => rendererWasmUrl,
  });

  initialized = true;
}

export async function renderTypstToSvg(source: string): Promise<string> {
  ensureInitialized();

  return $typst.svg({ mainContent: source });
}

export async function renderTypstToSvgWithTheme(
  source: string,
  options: TypstRenderOptions = {},
): Promise<string> {
  ensureInitialized();

  const darkMode = options.darkMode ?? false;
  const pageWidth = options.pageWidth ?? "auto";
  const pageHeight = options.pageHeight ?? "auto";
  const textColor = darkMode ? "#edf2ff" : "#111111";

  const themedSource = [
    `#set page(width: ${pageWidth}, height: ${pageHeight}, margin: 0pt)`,
    `#set text(size: 30pt, fill: rgb("${textColor}"))`,
    "",
    source,
  ].join("\n");

  return $typst.svg({ mainContent: themedSource });
}
