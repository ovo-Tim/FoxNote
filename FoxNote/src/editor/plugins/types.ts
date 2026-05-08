import type { Component } from "vue";
import type { NoteBlock } from "../../types/note";
import type { NoteRecord } from "../../types/note";

export interface BlockRenderContext {
  block: NoteBlock;
  note: NoteRecord | null;
  index: number;
  editing: boolean;
  setEditing: () => void;
  clearEditing: () => void;
  updateBlock: (nextBlock: NoteBlock) => void;
  updateContent: (nextValue: string) => void;
  updatePath: (nextPath: string) => void;
  updateSize: (nextWidth: number, nextHeight: number) => void;
  updateLevel: (nextLevel: number) => void;
  updateFolded: (nextFolded: boolean) => void;
  updateSummary: (nextSummary: string) => void;
}

export type BlockExportFormat = "svg" | "png";

export interface BlockExportResult {
  format: BlockExportFormat;
  mimeType: string;
  bytes: Uint8Array;
  width: number;
  height: number;
}

export interface BlockExportContext {
  block: NoteBlock;
  note: NoteRecord | null;
  index: number;
}

export interface BlockTypstAsset {
  fileName: string;
  mimeType: string;
  bytes: Uint8Array;
}

export interface BlockTypstExportResult {
  typst: string;
  assets?: BlockTypstAsset[];
}

export interface BlockRendererDefinition {
  component: Component;
  props?: (context: BlockRenderContext) => Record<string, unknown>;
  on?: (context: BlockRenderContext) => Record<string, (...args: any[]) => void>;
}

export interface BlockPlugin {
  type: string;
  label: string;
  description: string;
  createDefaultBlock: () => NoteBlock;
  renderer?: BlockRendererDefinition;
  previewText?: (block: NoteBlock) => string;
  exportBlock?: (
    context: BlockExportContext,
    format: BlockExportFormat,
  ) => Promise<BlockExportResult>;
  exportTypst?: (context: BlockExportContext) => Promise<BlockTypstExportResult>;
}

export interface EditorPlugin {
  id: string;
  name?: string;
  blocks: BlockPlugin[];
}
