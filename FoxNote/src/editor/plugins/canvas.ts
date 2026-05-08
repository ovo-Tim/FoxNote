import CanvasBlockCard from "../../components/blocks/CanvasBlockCard.vue";
import { exportCanvasAttachment } from "../../lib/blockExport";
import type { NoteBlock } from "../../types/note";
import type { BlockPlugin, EditorPlugin } from "./types";

function escapeTypstPath(input: string): string {
  return input.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function sanitizeAssetFileName(input: string, fallback: string): string {
  const normalized = input
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return normalized || fallback;
}

function toTypstWidthPt(width: number): number {
  const widthPt = Math.round(width * 0.75);
  return Math.max(120, Math.min(460, widthPt));
}

function buildTypstImageCall(fileName: string, width?: number | null): string {
  const args = [`"${escapeTypstPath(fileName)}"`];
  if (typeof width === "number" && width > 0) {
    args.push(`width: ${toTypstWidthPt(width)}pt`);
  } else {
    args.push("width: 100%");
  }
  return `#image(${args.join(", ")})`;
}

const canvasBlockPlugin: BlockPlugin = {
  type: "canvas",
  label: "Canvas",
  description: "tldraw whiteboard canvas",
  createDefaultBlock: (): NoteBlock => ({
    type: "canvas",
    path: "",
  }),
  previewText: (block) => {
    const path = block.path?.trim() ?? "";
    return path ? `[canvas] ${path}` : "[canvas] unsaved";
  },
  renderer: {
    component: CanvasBlockCard,
    props: (context) => ({
      noteId: context.note?.id ?? "",
      path: String(context.block.path ?? ""),
      editing: context.editing,
      width: Number(context.block.width ?? 0),
      height: Number(context.block.height ?? 0),
    }),
    on: (context) => ({
      focus: context.setEditing,
      updatePath: (path: string) => context.updatePath(String(path ?? "")),
      updateSize: (width: number, height: number) => context.updateSize(width, height),
    }),
  },
  exportBlock: async (context, format) => {
    const noteId = context.note?.id ?? "";
    const path = String(context.block.path ?? "").trim();
    if (!noteId || !path) {
      throw new Error("Canvas block has no attachment path.");
    }

    const result = await exportCanvasAttachment(noteId, path, format);
    return {
      format,
      mimeType: result.mimeType,
      bytes: result.bytes,
      width: result.width,
      height: result.height,
    };
  },
  exportTypst: async (context) => {
    const noteId = context.note?.id ?? "";
    const path = String(context.block.path ?? "").trim();
    if (!noteId || !path) {
      return { typst: "#text(fill: rgb(160, 0, 0))[Missing canvas attachment]" };
    }

    const result = await exportCanvasAttachment(noteId, path, "svg");
    const fileName = sanitizeAssetFileName(`canvas-${context.index + 1}.svg`, `canvas-${context.index + 1}.svg`);

    return {
      typst: buildTypstImageCall(fileName, context.block.width || result.width),
      assets: [
        {
          fileName,
          mimeType: "image/svg+xml",
          bytes: result.bytes,
        },
      ],
    };
  },
};

export const canvasEditorPlugin: EditorPlugin = {
  id: "builtin.canvas",
  name: "Canvas",
  blocks: [canvasBlockPlugin],
};

export { canvasBlockPlugin };
export default canvasEditorPlugin;
