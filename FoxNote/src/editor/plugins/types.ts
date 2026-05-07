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
  updateLevel: (nextLevel: number) => void;
  updateFolded: (nextFolded: boolean) => void;
  updateSummary: (nextSummary: string) => void;
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
}

export interface EditorPlugin {
  id: string;
  name?: string;
  blocks: BlockPlugin[];
}
