import ImageBlockCard from "../../components/blocks/ImageBlockCard.vue";
import { exportImageAttachment, exportTextAsGraphic } from "../../lib/blockExport";
import { loadNoteAttachment } from "../../lib/noteApi";
import type { NoteBlock } from "../../types/note";
import type { BlockPlugin, EditorPlugin } from "./types";

function sanitizeAssetFileName(input: string, fallback: string): string {
  const normalized = input
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return normalized || fallback;
}

function escapeTypstPath(input: string): string {
  return input.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function escapeTypstText(input: string): string {
  return input.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function resolveAttachmentExtension(path: string, mimeType: string): string {
  const fromPath = path.split("/").pop()?.split(".").pop()?.trim().toLowerCase();
  if (fromPath) {
    return fromPath;
  }

  if (/svg/i.test(mimeType)) {
    return "svg";
  }
  if (/png/i.test(mimeType)) {
    return "png";
  }
  if (/jpe?g/i.test(mimeType)) {
    return "jpg";
  }
  if (/gif/i.test(mimeType)) {
    return "gif";
  }
  if (/webp/i.test(mimeType)) {
    return "webp";
  }
  return "bin";
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

const imageBlockPlugin: BlockPlugin = {
  type: "image",
  label: "Image",
  description: "Image attachment preview",
  createDefaultBlock: (): NoteBlock => ({
    type: "image",
    path: "",
  }),
  previewText: (block) => {
    if (block.path?.trim()) {
      return `[image] ${block.path}`;
    }
    return "[image] attachment missing";
  },
  renderer: {
    component: ImageBlockCard,
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
      throw new Error("Image block has no attachment path.");
    }

    const result = await exportImageAttachment(noteId, path, format);
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
      return { typst: "#text(fill: rgb(160, 0, 0))[Missing image attachment]" };
    }

    const payload = await loadNoteAttachment(noteId, path);
    const bytes = Uint8Array.from(payload.bytes ?? []);
    const extension = resolveAttachmentExtension(path, payload.mimeType || "application/octet-stream");
    const fileName = sanitizeAssetFileName(`image-${context.index + 1}.${extension}`, `image-${context.index + 1}.${extension}`);

    return {
      typst: buildTypstImageCall(fileName, context.block.width),
      assets: [
        {
          fileName,
          mimeType: payload.mimeType || "application/octet-stream",
          bytes,
        },
      ],
    };
  },
};

const imageCaptionBlockPlugin: BlockPlugin = {
  type: "image_caption",
  label: "Image caption",
  description: "Caption text for nearby images",
  createDefaultBlock: (): NoteBlock => ({
    type: "image_caption",
    content: "Caption",
  }),
  previewText: (block) => {
    const content = typeof block.content === "string" ? block.content.trim() : "";
    return content ? `[caption] ${content}` : "[caption]";
  },
  exportBlock: async (context, format) => {
    const text = typeof context.block.content === "string" ? context.block.content.trim() : "";
    const result = await exportTextAsGraphic(text || "Image caption", format, {
      width: 960,
      height: 96,
      fontSize: 26,
      fontWeight: 500,
      color: "#243246",
      background: "transparent",
    });
    return {
      format,
      mimeType: result.mimeType,
      bytes: result.bytes,
      width: result.width,
      height: result.height,
    };
  },
  exportTypst: async (context) => {
    const text = typeof context.block.content === "string" ? context.block.content.trim() : "";
    return {
      typst: text ? `#text(size: 10pt, fill: rgb("#4b5563"), style: "italic")[${escapeTypstText(text)}]` : "",
    };
  },
};

export const imageEditorPlugin: EditorPlugin = {
  id: "builtin.image",
  name: "Image",
  blocks: [imageBlockPlugin, imageCaptionBlockPlugin],
};

export { imageBlockPlugin, imageCaptionBlockPlugin };
export default imageEditorPlugin;
