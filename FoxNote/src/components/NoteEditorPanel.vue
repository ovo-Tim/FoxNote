<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { saveNoteImageAttachment } from "../lib/noteApi";
import UnsupportedBlockCard from "./blocks/UnsupportedBlockCard.vue";
import { createDefaultBlockForType, getBlockPlugin, listBlockPlugins } from "../editor/plugins/registry";
import { cloneBlocks } from "../editor/utils";
import type { NoteBlock, NoteDocument, NoteRecord } from "../types/note";
import type { BlockRenderContext } from "../editor/plugins/types";

const props = defineProps<{
  note: NoteRecord | null;
  tagOptions?: string[];
}>();

const emit = defineEmits<{
  change: [document: NoteDocument];
}>();

const form = reactive<NoteDocument>({
  title: "",
  date: "",
  type: "notes",
  tags: [],
  content: [],
});

const tagMenuOpen = ref(false);
const tagQuery = ref("");
const insertMenuIndex = ref<number | null>(null);
const slashQuery = ref("");
const actionMenuIndex = ref<number | null>(null);
const actionQuery = ref("");
const editingBlockIndex = ref<number | null>(null);
const isHydratingForm = ref(false);
const titleEditing = ref(false);
const titleInputRef = ref<HTMLInputElement | null>(null);
const editorPaneRef = ref<HTMLElement | null>(null);
const blockStackRef = ref<HTMLElement | null>(null);
const blockElements = ref<(HTMLElement | null)[]>([]);
const reorderState = reactive({
  active: false,
  sourceIndex: -1,
  targetIndex: -1,
  startX: 0,
  startY: 0,
  isDragging: false,
  justDragged: false,
});
const selectedBlocks = ref<number[]>([]);
const selectionAnchor = ref<number | null>(null);
const marquee = reactive({
  active: false,
  pending: false,
  justFinished: false,
  startX: 0,
  startY: 0,
  currentX: 0,
  currentY: 0,
});

const tagToneClasses = ["tone-amber", "tone-orange", "tone-gray", "tone-cyan", "tone-magenta"];
const blockPlugins = listBlockPlugins();

const availableTagOptions = computed(() => {
  const base = props.tagOptions ?? [];
  const current = form.tags;
  const merged = new Set<string>([...base, ...current]);
  return [...merged].sort((left, right) => left.localeCompare(right));
});

const noteTypeLabel = computed(() => {
  if (form.type === "notes") {
    return "Page";
  }
  return form.type || "Page";
});

const normalizedTagQuery = computed(() => {
  return normalizeTagCandidate(tagQuery.value);
});

const filteredTagOptions = computed(() => {
  const query = normalizedTagQuery.value.toLowerCase();
  if (!query) {
    return availableTagOptions.value;
  }

  return availableTagOptions.value.filter((option) => option.toLowerCase().includes(query));
});

const showCreateTagOption = computed(() => {
  const candidate = normalizedTagQuery.value;
  return candidate.length > 0 && !availableTagOptions.value.includes(candidate);
});

const blockPaletteOptions = computed(() => {
  const query = slashQuery.value.trim().toLowerCase();
  if (!query) {
    return blockPlugins;
  }

  return blockPlugins.filter((plugin) => {
    const indexText = `${plugin.label} ${plugin.type} ${plugin.description}`.toLowerCase();
    return indexText.includes(query);
  });
});

const filteredBlockActions = computed(() => {
  const query = actionQuery.value.trim().toLowerCase();
  const actions = [
    { key: "duplicate", label: "Duplicate block", hotkey: "Cmd + D" },
    { key: "delete", label: "Delete block", hotkey: "Del" },
  ];

  if (!query) {
    return actions;
  }

  return actions.filter((action) => action.label.toLowerCase().includes(query));
});

const selectedBlockSet = computed(() => {
  return new Set(selectedBlocks.value);
});

const hiddenBlockSet = computed(() => {
  const hidden = new Set<number>();
  const foldedLevels: number[] = [];

  for (let index = 0; index < form.content.length; index += 1) {
    const block = form.content[index];
    if (!block) {
      continue;
    }

    if (block.type === "title") {
      const level = Math.min(6, Math.max(1, Number(block.level) || 1));
      while (foldedLevels.length > 0 && level <= (foldedLevels[foldedLevels.length - 1] ?? 0)) {
        foldedLevels.pop();
      }

      if (foldedLevels.length > 0) {
        hidden.add(index);
      }

      if (Boolean(block.folded)) {
        foldedLevels.push(level);
      }
      continue;
    }

    if (foldedLevels.length > 0) {
      hidden.add(index);
    }
  }

  return hidden;
});

const blockIndentLevelMap = computed(() => {
  const levels = new Map<number, number>();
  const stack: Array<{ level: number; depth: number }> = [];

  for (let index = 0; index < form.content.length; index += 1) {
    const block = form.content[index];
    if (!block) {
      continue;
    }

    if (block.type !== "title") {
      const activeTitle = stack[stack.length - 1];
      levels.set(index, activeTitle ? activeTitle.depth + 1 : 0);
      continue;
    }

    const level = Math.min(6, Math.max(1, Number(block.level) || 1));
    while (stack.length > 0 && level <= (stack[stack.length - 1]?.level ?? 0)) {
      stack.pop();
    }

    const parent = stack[stack.length - 1];
    const depth = parent ? parent.depth + 1 : 0;
    levels.set(index, depth);
    stack.push({ level, depth });
  }

  return levels;
});

const marqueeStyle = computed<Record<string, string>>(() => {
  const hiddenStyle: Record<string, string> = {
    display: "none",
    left: "0px",
    top: "0px",
    width: "0px",
    height: "0px",
  };

  const paneEl = editorPaneRef.value;
  if (!marquee.active || !paneEl) {
    return hiddenStyle;
  }

  const bounds = paneEl.getBoundingClientRect();
  const left = Math.min(marquee.startX, marquee.currentX);
  const top = Math.min(marquee.startY, marquee.currentY);
  const right = Math.max(marquee.startX, marquee.currentX);
  const bottom = Math.max(marquee.startY, marquee.currentY);

  return {
    display: "block",
    left: `${left - bounds.left}px`,
    top: `${top - bounds.top}px`,
    width: `${Math.max(0, right - left)}px`,
    height: `${Math.max(0, bottom - top)}px`,
  };
});

function normalizeTagInput(values: unknown) {
  if (!Array.isArray(values)) {
    form.tags = [];
    return;
  }

  form.tags = values
    .filter((value): value is string => typeof value === "string")
    .map((value) => value.trim())
    .filter(Boolean)
    .filter((value, index, array) => array.indexOf(value) === index);
}

function normalizeTagCandidate(raw: string): string {
  return raw.trim().replace(/\s+/g, "-").replace(/\/{2,}/g, "/").replace(/^\/+|\/+$/g, "");
}

function ensureAtLeastOneBlock() {
  if (form.content.length > 0) {
    return;
  }

  const defaultBlock = createDefaultBlockForType("typst") ?? {
    type: "typst",
    content: "",
  };
  form.content = [defaultBlock];
}

function isTagSelected(tag: string): boolean {
  return form.tags.includes(tag);
}

function toggleTag(tag: string) {
  if (isTagSelected(tag)) {
    normalizeTagInput(form.tags.filter((item) => item !== tag));
    return;
  }

  normalizeTagInput([...form.tags, tag]);
}

function createTagFromQuery() {
  const candidate = normalizedTagQuery.value;
  if (!candidate) {
    return;
  }

  if (!isTagSelected(candidate)) {
    normalizeTagInput([...form.tags, candidate]);
  }
  tagQuery.value = "";
}

function tagToneClass(tag: string): string {
  let hash = 0;
  for (const char of tag) {
    hash = (hash << 5) - hash + char.charCodeAt(0);
    hash |= 0;
  }
  return tagToneClasses[Math.abs(hash) % tagToneClasses.length] ?? "tone-gray";
}

function updateBlock(index: number, nextBlock: NoteBlock) {
  const blocks = cloneBlocks(form.content);
  if (!blocks[index]) {
    return;
  }
  blocks[index] = nextBlock;
  form.content = blocks;
}

function updateBlockPath(index: number, nextPath: string) {
  const block = form.content[index];
  if (!block) {
    return;
  }

  const trimmed = nextPath.trim();
  if (!trimmed || trimmed === block.path) {
    return;
  }

  updateBlock(index, {
    ...block,
    path: trimmed,
  });
}

function updateBlockContent(index: number, nextValue: string) {
  const block = form.content[index];
  if (!block) {
    return;
  }

  const normalized = String(nextValue ?? "");
  if ((block.content ?? "") === normalized) {
    return;
  }

  updateBlock(index, {
    ...block,
    content: normalized,
  });
}

function updateBlockLevel(index: number, nextLevel: number) {
  const block = form.content[index];
  if (!block) {
    return;
  }

  const normalized = Math.min(6, Math.max(1, Math.round(Number(nextLevel) || 1)));
  if (block.level === normalized) {
    return;
  }

  updateBlock(index, {
    ...block,
    level: normalized,
  });
}

function updateBlockFolded(index: number, nextFolded: boolean) {
  const block = form.content[index];
  if (!block) {
    return;
  }

  const normalized = Boolean(nextFolded);
  if (Boolean(block.folded) === normalized) {
    return;
  }

  updateBlock(index, {
    ...block,
    folded: normalized,
  });
}

function updateBlockSummary(index: number, nextSummary: string) {
  const block = form.content[index];
  if (!block) {
    return;
  }

  const normalized = nextSummary;
  if ((block.summary ?? "") === normalized) {
    return;
  }

  updateBlock(index, {
    ...block,
    summary: normalized,
  });
}

function buildBlockRenderContext(index: number, block: NoteBlock): BlockRenderContext {
  return {
    block,
    note: props.note,
    index,
    editing: editingBlockIndex.value === index,
    setEditing: () => setEditingBlock(index),
    clearEditing: () => finishBlockEditing(index),
    updateBlock: (nextBlock) => updateBlock(index, nextBlock),
    updateContent: (nextValue) => {
      if (block.type === "typst") {
        onBlockInput(index, nextValue);
        return;
      }
      updateBlockContent(index, nextValue);
    },
    updatePath: (nextPath) => updateBlockPath(index, nextPath),
    updateLevel: (nextLevel) => updateBlockLevel(index, nextLevel),
    updateFolded: (nextFolded) => updateBlockFolded(index, nextFolded),
    updateSummary: (nextSummary) => updateBlockSummary(index, nextSummary),
  };
}

function parseTypstTitleCandidate(content: string): { level: number; title: string; body: string } | null {
  const normalized = content.replace(/\r\n/g, "\n");
  const lines = normalized.split("\n");
  let firstNonEmpty = -1;

  for (let cursor = 0; cursor < lines.length; cursor += 1) {
    if (lines[cursor]?.trim()) {
      firstNonEmpty = cursor;
      break;
    }
  }

  if (firstNonEmpty < 0) {
    return null;
  }

  const headingLine = lines[firstNonEmpty]?.trim() ?? "";
  const match = headingLine.match(/^(={1,6})\s+(.+)$/);
  if (!match) {
    return null;
  }

  const title = (match[2] ?? "").trim();
  if (!title) {
    return null;
  }

  const remainingLines = lines.slice(firstNonEmpty + 1);
  const body = remainingLines.join("\n").trim();
  return {
    level: (match[1] ?? "=").length,
    title,
    body,
  };
}

function splitTypstHeadingBlock(index: number, content: string): boolean {
  const block = form.content[index];
  if (!block || block.type !== "typst") {
    return false;
  }

  const parsed = parseTypstTitleCandidate(content);
  if (!parsed) {
    return false;
  }

  const blocks = cloneBlocks(form.content);
  blocks[index] = {
    type: "title",
    level: parsed.level,
    content: parsed.title,
    folded: false,
    summary: "",
  };

  if (parsed.body) {
    blocks.splice(index + 1, 0, {
      type: "typst",
      content: parsed.body,
    });
  }

  form.content = blocks;
  closeSlashMenu();
  closeActionMenu();
  return true;
}

function finishBlockEditing(index: number) {
  const block = form.content[index];
  if (block?.type === "typst") {
    splitTypstHeadingBlock(index, typeof block.content === "string" ? block.content : "");
  }

  if (editingBlockIndex.value === index) {
    setEditingBlock(null);
  }
}

function resolveBlockComponent(block: NoteBlock) {
  return getBlockPlugin(block.type)?.renderer?.component ?? UnsupportedBlockCard;
}

function resolveBlockProps(index: number, block: NoteBlock): Record<string, unknown> {
  const renderer = getBlockPlugin(block.type)?.renderer;
  if (!renderer) {
    return { block };
  }

  if (!renderer.props) {
    return {};
  }

  return renderer.props(buildBlockRenderContext(index, block));
}

function resolveBlockListeners(index: number, block: NoteBlock): Record<string, (...args: any[]) => void> {
  const renderer = getBlockPlugin(block.type)?.renderer;
  if (!renderer || !renderer.on) {
    return {};
  }

  return renderer.on(buildBlockRenderContext(index, block));
}

function setBlockElement(index: number, element: unknown) {
  const candidate = element instanceof HTMLElement ? element : null;
  if (!candidate) {
    blockElements.value[index] = null;
    return;
  }
  blockElements.value[index] = candidate;
}

function isBlockSelected(index: number): boolean {
  return selectedBlockSet.value.has(index);
}

function isBlockHidden(index: number): boolean {
  return hiddenBlockSet.value.has(index);
}

function blockShellStyle(index: number): Record<string, string> {
  const depth = blockIndentLevelMap.value.get(index) ?? 0;
  return {
    marginLeft: `${depth * 1.1}rem`,
  };
}

function normalizeSelection(indices: number[]) {
  const max = form.content.length - 1;
  selectedBlocks.value = [...new Set(indices)].filter((index) => index >= 0 && index <= max).sort((a, b) => a - b);
}

function selectOnly(index: number) {
  normalizeSelection([index]);
  selectionAnchor.value = index;
}

function toggleBlockSelection(index: number) {
  if (isBlockSelected(index)) {
    normalizeSelection(selectedBlocks.value.filter((value) => value !== index));
  } else {
    normalizeSelection([...selectedBlocks.value, index]);
  }
  selectionAnchor.value = index;
}

function selectRangeTo(index: number) {
  const anchor = selectionAnchor.value ?? index;
  const start = Math.min(anchor, index);
  const end = Math.max(anchor, index);
  const range: number[] = [];
  for (let cursor = start; cursor <= end; cursor += 1) {
    range.push(cursor);
  }
  normalizeSelection(range);
}

function onBlockMouseDown(index: number, event: MouseEvent) {
  if (marquee.pending || marquee.active) {
    return;
  }

  if (event.button !== 0) {
    return;
  }

  if (event.shiftKey) {
    selectRangeTo(index);
    return;
  }

  if (event.metaKey || event.ctrlKey) {
    toggleBlockSelection(index);
    return;
  }

  selectOnly(index);
}

function setEditingBlock(index: number | null) {
  editingBlockIndex.value = index;
}

function openSlashMenu(index: number) {
  insertMenuIndex.value = index;
  slashQuery.value = "";
}

function closeSlashMenu() {
  insertMenuIndex.value = null;
  slashQuery.value = "";
}

function openActionMenu(index: number) {
  if (!isBlockSelected(index)) {
    selectOnly(index);
  }
  actionMenuIndex.value = index;
  actionQuery.value = "";
}

function closeActionMenu() {
  actionMenuIndex.value = null;
  actionQuery.value = "";
}

function insertBlockAfter(index: number, blockType: string) {
  const newBlock = createDefaultBlockForType(blockType);
  if (!newBlock) {
    return;
  }

  const blocks = cloneBlocks(form.content);
  blocks.splice(index + 1, 0, newBlock);
  form.content = blocks;
  setEditingBlock(index + 1);
  closeSlashMenu();
}

function onBlockInput(index: number, nextValue: string) {
  const block = form.content[index];
  if (!block) {
    return;
  }

  updateBlockContent(index, nextValue);

  if (block.type === "typst" && nextValue.trim() === "/") {
    updateBlockContent(index, "");
    openSlashMenu(index);
  }
}

function removeBlock(index: number) {
  const blocks = cloneBlocks(form.content);
  const targets = isBlockSelected(index) ? [...selectedBlocks.value] : [index];

  if (targets.length === 0) {
    return;
  }

  for (const target of [...targets].sort((a, b) => b - a)) {
    if (blocks[target]) {
      blocks.splice(target, 1);
    }
  }

  form.content = blocks;
  ensureAtLeastOneBlock();
  normalizeSelection([]);
  selectionAnchor.value = null;
  setEditingBlock(null);
  closeSlashMenu();
  closeActionMenu();
}

function duplicateBlock(index: number) {
  const sourceIndices = isBlockSelected(index) ? [...selectedBlocks.value] : [index];
  if (sourceIndices.length === 0) {
    return;
  }

  const blocks = cloneBlocks(form.content);
  const inserted: number[] = [];
  let offset = 0;

  for (const sourceIndex of [...sourceIndices].sort((a, b) => a - b)) {
    const block = blocks[sourceIndex + offset];
    if (!block) {
      continue;
    }
    const insertIndex = sourceIndex + offset + 1;
    blocks.splice(insertIndex, 0, { ...block });
    inserted.push(insertIndex);
    offset += 1;
  }

  form.content = blocks;
  normalizeSelection(inserted);
  selectionAnchor.value = inserted[inserted.length - 1] ?? null;
  setEditingBlock(inserted[inserted.length - 1] ?? null);
  closeActionMenu();
}

function startTitleEdit() {
  titleEditing.value = true;
  void nextTick(() => {
    titleInputRef.value?.focus();
    titleInputRef.value?.select();
  });
}

function finishTitleEdit() {
  titleEditing.value = false;
  if (!form.title.trim()) {
    form.title = "Untitled Note";
  }
}

function exitBlockEditMode() {
  if (editingBlockIndex.value !== null) {
    finishBlockEditing(editingBlockIndex.value);
  }
  clearSelection();
  closeSlashMenu();
  closeActionMenu();
}

function clearSelection() {
  normalizeSelection([]);
  selectionAnchor.value = null;
}

function resetReorderState() {
  reorderState.active = false;
  reorderState.sourceIndex = -1;
  reorderState.targetIndex = -1;
  reorderState.startX = 0;
  reorderState.startY = 0;
  reorderState.isDragging = false;
}

function onMenuHandlePointerDown(index: number, event: PointerEvent) {
  event.stopPropagation();
  if (event.button !== 0) {
    return;
  }

  selectOnly(index);

  reorderState.active = true;
  reorderState.sourceIndex = index;
  reorderState.targetIndex = index;
  reorderState.startX = event.clientX;
  reorderState.startY = event.clientY;
  reorderState.isDragging = false;
  reorderState.justDragged = false;

  window.addEventListener("pointermove", onMenuHandlePointerMove);
  window.addEventListener("pointerup", onMenuHandlePointerUp);
}

function onMenuHandlePointerMove(event: PointerEvent) {
  if (!reorderState.active || reorderState.sourceIndex < 0) {
    return;
  }

  const distance = Math.hypot(event.clientX - reorderState.startX, event.clientY - reorderState.startY);
  if (distance > 4) {
    reorderState.isDragging = true;
  }

  if (!reorderState.isDragging) {
    return;
  }

  event.preventDefault();

  let hoveredIndex = -1;
  for (let index = 0; index < blockElements.value.length; index += 1) {
    const element = blockElements.value[index];
    if (!element) {
      continue;
    }

    const rect = element.getBoundingClientRect();
    if (event.clientY >= rect.top && event.clientY <= rect.bottom) {
      const middle = rect.top + rect.height / 2;
      hoveredIndex = event.clientY < middle ? index : index + 1;
      break;
    }
  }

  if (hoveredIndex >= 0) {
    reorderState.targetIndex = hoveredIndex;
    return;
  }

  const stackRect = blockStackRef.value?.getBoundingClientRect();
  if (!stackRect) {
    return;
  }

  if (event.clientY > stackRect.bottom) {
    reorderState.targetIndex = form.content.length;
  } else if (event.clientY < stackRect.top) {
    reorderState.targetIndex = 0;
  }
}

function onMenuHandlePointerUp(event: PointerEvent) {
  event.preventDefault();
  event.stopPropagation();

  window.removeEventListener("pointermove", onMenuHandlePointerMove);
  window.removeEventListener("pointerup", onMenuHandlePointerUp);

  if (!reorderState.active) {
    return;
  }

  const sourceIndex = reorderState.sourceIndex;
  const targetIndex = reorderState.targetIndex;
  const shouldMove =
    reorderState.isDragging &&
    sourceIndex >= 0 &&
    targetIndex >= 0 &&
    targetIndex <= form.content.length &&
    targetIndex !== sourceIndex;

  if (shouldMove) {
    const blocks = cloneBlocks(form.content);
    const moved = blocks[sourceIndex];
    if (moved) {
      blocks.splice(sourceIndex, 1);
      const insertAt = sourceIndex < targetIndex ? targetIndex - 1 : targetIndex;
      blocks.splice(insertAt, 0, moved);
      form.content = blocks;
      selectOnly(insertAt);
      reorderState.justDragged = true;
    }
  } else {
    reorderState.justDragged = false;
  }

  resetReorderState();
}

function onMenuHandleClick(index: number, event: MouseEvent) {
  event.stopPropagation();

  if (reorderState.justDragged) {
    reorderState.justDragged = false;
    return;
  }

  openActionMenu(index);
}

function shouldShowDropIndicator(): boolean {
  return reorderState.active && reorderState.isDragging && reorderState.targetIndex >= 0;
}

function isDropTargetTop(index: number): boolean {
  return shouldShowDropIndicator() && reorderState.targetIndex === index;
}

function isDropTargetBottom(index: number): boolean {
  return (
    shouldShowDropIndicator() &&
    reorderState.targetIndex === form.content.length &&
    index === form.content.length - 1
  );
}

function isTextEntryTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  if (target.closest("input, textarea, [contenteditable='true']")) {
    return true;
  }

  return Boolean(target.closest(".v-field"));
}

function resolveTargetElement(target: EventTarget | null): HTMLElement | null {
  if (target instanceof HTMLElement) {
    return target;
  }
  if (target instanceof Node) {
    return target.parentElement;
  }
  return null;
}

function onKeydown(event: KeyboardEvent) {
  if (!props.note) {
    return;
  }

  const targetEl = resolveTargetElement(event.target);
  const withinEditor = Boolean(targetEl && editorPaneRef.value?.contains(targetEl));

  if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && withinEditor) {
    event.preventDefault();

    if (editingBlockIndex.value !== null) {
      finishBlockEditing(editingBlockIndex.value);
    }

    if (titleEditing.value) {
      finishTitleEdit();
    }

    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }

    closeSlashMenu();
    closeActionMenu();
    return;
  }

  if (selectedBlocks.value.length === 0) {
    return;
  }

  if (isTextEntryTarget(event.target)) {
    return;
  }

  if ((event.key === "Backspace" || event.key === "Delete") && selectedBlocks.value.length > 0) {
    event.preventDefault();
    removeBlock(selectedBlocks.value[0] ?? 0);
    return;
  }

  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "d") {
    event.preventDefault();
    duplicateBlock(selectedBlocks.value[selectedBlocks.value.length - 1] ?? 0);
    return;
  }

  if (event.key === "Escape") {
    clearSelection();
  }
}

function insertTypstBlockFromPaste(text: string) {
  const pasted = text.replace(/\r\n/g, "\n");
  if (!pasted.trim()) {
    return;
  }

  const blocks = cloneBlocks(form.content);
  const baseIndex = selectedBlocks.value[selectedBlocks.value.length - 1] ?? form.content.length - 1;
  const insertIndex = Math.max(0, Math.min(blocks.length, baseIndex + 1));

  const parsedTitle = parseTypstTitleCandidate(pasted);
  if (parsedTitle) {
    blocks.splice(insertIndex, 0, {
      type: "title",
      level: parsedTitle.level,
      content: parsedTitle.title,
      folded: false,
      summary: "",
    });

    if (parsedTitle.body) {
      blocks.splice(insertIndex + 1, 0, {
        type: "typst",
        content: parsedTitle.body,
      });
      form.content = blocks;
      selectOnly(insertIndex + 1);
      setEditingBlock(insertIndex + 1);
    } else {
      form.content = blocks;
      selectOnly(insertIndex);
      setEditingBlock(insertIndex);
    }
    closeSlashMenu();
    closeActionMenu();
    return;
  }

  blocks.splice(insertIndex, 0, {
    type: "typst",
    content: pasted,
  });

  form.content = blocks;
  selectOnly(insertIndex);
  setEditingBlock(insertIndex);
  closeSlashMenu();
  closeActionMenu();
}

async function insertImageBlockFromPaste(file: File) {
  if (!props.note) {
    return;
  }

  const bytes = [...new Uint8Array(await file.arrayBuffer())];
  const attachmentPath = await saveNoteImageAttachment(
    props.note.id,
    file.type || "image/png",
    bytes,
  );

  const blocks = cloneBlocks(form.content);
  const baseIndex = selectedBlocks.value[selectedBlocks.value.length - 1] ?? form.content.length - 1;
  const insertIndex = Math.max(0, Math.min(blocks.length, baseIndex + 1));

  blocks.splice(insertIndex, 0, {
    type: "image",
    path: attachmentPath,
  });

  form.content = blocks;
  selectOnly(insertIndex);
  setEditingBlock(insertIndex);
  closeSlashMenu();
  closeActionMenu();
}

async function onWindowPaste(event: ClipboardEvent) {
  if (!props.note) {
    return;
  }

  if (isTextEntryTarget(event.target)) {
    return;
  }

  const imageItem = [...(event.clipboardData?.items ?? [])].find((item) => item.type.startsWith("image/"));
  const imageFile = imageItem?.getAsFile() ?? null;
  if (imageFile) {
    event.preventDefault();
    try {
      await insertImageBlockFromPaste(imageFile);
    } catch (reason) {
      console.error("failed to paste image", reason);
    }
    return;
  }

  const pastedText = event.clipboardData?.getData("text/plain") ?? "";
  if (!pastedText.trim()) {
    return;
  }

  event.preventDefault();
  insertTypstBlockFromPaste(pastedText);
}

function intersects(a: DOMRect, b: DOMRect): boolean {
  return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
}

function setGlobalMarqueeTextSelection(disabled: boolean) {
  document.body.classList.toggle("fox-marquee-active", disabled);
}

function updateSelectionFromMarquee() {
  if (!marquee.active) {
    return;
  }

  const selectionRect = new DOMRect(
    Math.min(marquee.startX, marquee.currentX),
    Math.min(marquee.startY, marquee.currentY),
    Math.abs(marquee.currentX - marquee.startX),
    Math.abs(marquee.currentY - marquee.startY),
  );

  const hits: number[] = [];
  for (let index = 0; index < blockElements.value.length; index += 1) {
    const element = blockElements.value[index];
    if (!element) {
      continue;
    }

    const rect = element.getBoundingClientRect();
    if (intersects(selectionRect, rect)) {
      hits.push(index);
    }
  }

  normalizeSelection(hits);
  if (hits.length > 0) {
    selectionAnchor.value = hits[hits.length - 1] ?? null;
  }
}

function onWindowMouseMove(event: MouseEvent) {
  if (marquee.pending && !marquee.active) {
    const movedDistance = Math.hypot(event.clientX - marquee.startX, event.clientY - marquee.startY);
    if (movedDistance > 4) {
      marquee.pending = false;
      marquee.active = true;
      clearSelection();
      setGlobalMarqueeTextSelection(true);
    }
  }

  if (!marquee.active) {
    return;
  }

  event.preventDefault();
  marquee.currentX = event.clientX;
  marquee.currentY = event.clientY;
  updateSelectionFromMarquee();
}

function stopMarquee() {
  if (marquee.pending) {
    marquee.pending = false;
    setGlobalMarqueeTextSelection(false);
    marquee.justFinished = false;
  }

  if (!marquee.active) {
    return;
  }

  marquee.active = false;
  setGlobalMarqueeTextSelection(false);
  marquee.justFinished = true;
}

function onEditorPaneMouseDown(event: MouseEvent) {
  if (event.button !== 0) {
    return;
  }

  const targetEl = resolveTargetElement(event.target);
  if (!targetEl) {
    return;
  }

  if (
    targetEl.closest(
      "button, input, textarea, [contenteditable='true'], .v-field, .block-hover-actions, .tag-menu, .slash-menu, .action-menu, .image-paint-zone, .vp-editor, .vp-main, .vp-image, .vp-toolbar",
    )
  ) {
    return;
  }

  marquee.pending = true;
  marquee.active = false;
  marquee.justFinished = false;
  marquee.startX = event.clientX;
  marquee.startY = event.clientY;
  marquee.currentX = event.clientX;
  marquee.currentY = event.clientY;
  setGlobalMarqueeTextSelection(true);
  event.preventDefault();
}

function onEditorPaneClickCapture(event: MouseEvent) {
  if (!marquee.justFinished) {
    return;
  }

  event.preventDefault();
  event.stopPropagation();
  marquee.justFinished = false;
}

watch(
  () => props.note,
  (note) => {
    isHydratingForm.value = true;

    if (!note) {
      form.title = "";
      form.date = "";
      form.type = "notes";
      form.tags = [];
      form.content = [];
      setEditingBlock(null);
      closeSlashMenu();
      closeActionMenu();
      titleEditing.value = false;
      isHydratingForm.value = false;
      return;
    }

    form.title = note.document.title;
    form.date = note.document.date;
    form.type = note.document.type;
    form.tags = [...note.document.tags];
    form.content = cloneBlocks(note.document.content);
    ensureAtLeastOneBlock();
    setEditingBlock(null);
    closeSlashMenu();
    closeActionMenu();
    titleEditing.value = false;
    clearSelection();
    isHydratingForm.value = false;
  },
  { immediate: true },
);

watch(
  () => form.content.length,
  () => {
    blockElements.value = blockElements.value.slice(0, form.content.length);
    normalizeSelection(selectedBlocks.value);
  },
);

watch(hiddenBlockSet, (hidden) => {
  const visibleSelection = selectedBlocks.value.filter((index) => !hidden.has(index));
  if (visibleSelection.length !== selectedBlocks.value.length) {
    normalizeSelection(visibleSelection);
    selectionAnchor.value = visibleSelection[visibleSelection.length - 1] ?? null;
  }

  if (editingBlockIndex.value !== null && hidden.has(editingBlockIndex.value)) {
    setEditingBlock(null);
  }
});

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("paste", onWindowPaste);
  window.addEventListener("mousemove", onWindowMouseMove);
  window.addEventListener("mouseup", stopMarquee);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onMenuHandlePointerMove);
  window.removeEventListener("pointerup", onMenuHandlePointerUp);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("paste", onWindowPaste);
  window.removeEventListener("mousemove", onWindowMouseMove);
  window.removeEventListener("mouseup", stopMarquee);
  setGlobalMarqueeTextSelection(false);
});

function emitChange() {
  if (!props.note || isHydratingForm.value) {
    return;
  }

  ensureAtLeastOneBlock();
  emit("change", {
    title: form.title,
    date: form.date,
    type: form.type,
    tags: [...form.tags],
    content: cloneBlocks(form.content),
  });
}

watch(form, emitChange, { deep: true });
</script>

<template>
  <section ref="editorPaneRef" class="editor-pane" :class="{ 'marquee-active': marquee.pending || marquee.active }"
    v-if="note" @click.self="exitBlockEditMode" @mousedown.capture="onEditorPaneMouseDown"
    @click.capture="onEditorPaneClickCapture">
    <header class="note-top">
      <div class="meta-wrap">
        <button v-if="!titleEditing" type="button" class="title-display" @click="startTitleEdit">
          {{ form.title || "Untitled" }}
        </button>
        <input v-else ref="titleInputRef" v-model="form.title" class="title-input" type="text" @blur="finishTitleEdit"
          @keydown.enter.prevent="finishTitleEdit" @keydown.esc.prevent="finishTitleEdit" />
        <p class="meta-line">
          <span>{{ noteTypeLabel }}</span>
          <span class="dot">•</span>
          <v-menu v-model="tagMenuOpen" location="bottom start" :close-on-content-click="false">
            <template #activator="{ props: menuProps }">
              <button type="button" class="tag-area-trigger" v-bind="menuProps">
                <template v-if="form.tags.length > 0">
                  <span v-for="tag in form.tags" :key="`meta-${tag}`" class="meta-tag-pill" :class="tagToneClass(tag)">
                    {{ tag }}
                  </span>
                </template>
                <span v-else class="empty-tag-label">No tags</span>
              </button>
            </template>

            <div class="tag-menu">
              <p class="tag-menu-label">Tag</p>
              <v-text-field v-model="tagQuery" density="compact" variant="solo-filled" hide-details rounded="pill"
                placeholder="Filter or create options..." class="tag-filter-input"
                @keydown.enter.prevent="createTagFromQuery" />

              <div class="tag-options">
                <button v-for="tag in filteredTagOptions" :key="`tag-option-${tag}`" type="button"
                  class="tag-option-row" @click="toggleTag(tag)">
                  <span class="drag-handle" aria-hidden="true">⋮⋮</span>
                  <span class="tag-pill" :class="tagToneClass(tag)">{{ tag }}</span>
                  <v-icon v-if="isTagSelected(tag)" icon="mdi-check" size="16" />
                </button>

                <button v-if="showCreateTagOption" type="button" class="tag-option-row create-row"
                  @click="createTagFromQuery">
                  <v-icon icon="mdi-plus-circle-outline" size="16" />
                  <span>Create "{{ normalizedTagQuery }}"</span>
                </button>
              </div>
            </div>
          </v-menu>
        </p>
      </div>
    </header>

    <section ref="blockStackRef" class="block-stack" @click.self="exitBlockEditMode">
      <article v-for="(block, index) in form.content" :key="`${note.id}-block-${index}-${block.type}`"
        v-show="!isBlockHidden(index)"
        :ref="(el) => setBlockElement(index, el)" class="block-shell" :class="{
          selected: isBlockSelected(index),
          'is-adding-top': isDropTargetTop(index),
          'is-adding-bottom': isDropTargetBottom(index),
        }" :style="blockShellStyle(index)" @mousedown="onBlockMouseDown(index, $event)">

        <component :is="resolveBlockComponent(block)" v-bind="resolveBlockProps(index, block)"
          v-on="resolveBlockListeners(index, block)" />

        <div class="block-hover-actions" @click.stop>
          <v-menu :model-value="insertMenuIndex === index" location="bottom start" :close-on-content-click="false"
            @update:model-value="(value) => !value && closeSlashMenu()">
            <template #activator="{ props: menuProps }">
              <button type="button" class="round-action-btn add-btn" v-bind="menuProps" @click="openSlashMenu(index)">
                <v-icon icon="mdi-plus" size="16" />
              </button>
            </template>

            <div class="slash-menu">
              <p class="slash-menu-title">Insert block</p>
              <v-text-field v-model="slashQuery" density="compact" variant="solo-filled" hide-details rounded="pill"
                placeholder="Filter actions..." class="slash-search" />
              <div class="slash-options">
                <button v-for="plugin in blockPaletteOptions" :key="plugin.type" type="button" class="slash-option"
                  @click="insertBlockAfter(index, plugin.type)">
                  <span class="slash-option-label">/{{ plugin.type }}</span>
                  <span class="slash-option-desc">{{ plugin.description }}</span>
                </button>
              </div>
            </div>
          </v-menu>

          <v-menu :model-value="actionMenuIndex === index" location="bottom start" :close-on-content-click="false"
            @update:model-value="(value) => !value && closeActionMenu()">
            <template #activator="{ props: menuProps }">
              <button type="button" class="round-action-btn menu-btn" v-bind="menuProps" title="Block actions / drag"
                @pointerdown="onMenuHandlePointerDown(index, $event)" @click="onMenuHandleClick(index, $event)">
                <v-icon icon="mdi-dots-vertical" size="16" />
              </button>
            </template>

            <div class="action-menu">
              <v-text-field v-model="actionQuery" density="compact" variant="solo-filled" hide-details rounded="pill"
                placeholder="Filter actions..." class="action-search" />

              <div class="action-group">
                <button v-for="action in filteredBlockActions" :key="action.key" type="button" class="action-row"
                  :class="{ danger: action.key === 'delete' }"
                  @click="action.key === 'delete' ? removeBlock(index) : duplicateBlock(index)">
                  <span>{{ action.label }}</span>
                  <span class="action-hotkey">{{ action.hotkey }}</span>
                </button>
              </div>
            </div>
          </v-menu>
        </div>
      </article>
    </section>

    <div class="marquee-selection" :style="marqueeStyle" />

  </section>

  <section class="editor-pane empty" v-else>
    <h2>Editor</h2>
    <p>Choose a note from the tree, or create a new one.</p>
  </section>
</template>

<style scoped>
.editor-pane {
  border: 0;
  border-radius: 0;
  background: transparent;
  padding: 0.65rem 0rem 0.85rem;
  width: min(100%, 980px);
  min-height: 100%;
  position: relative;
}

.editor-pane.marquee-active,
.editor-pane.marquee-active * {
  user-select: none;
}

.note-top {
  display: flex;
  align-items: flex-end;
  gap: 0.45rem;
}

.meta-wrap {
  flex: 1;
  min-width: 0;
  padding-left: 1.8rem;
}

.title-display {
  border: 0;
  background: transparent;
  padding: 0;
  margin: 0 0 0.15rem;
  font-family: inherit;
  font-size: 2.7rem;
  font-weight: 700;
  color: var(--fox-text-strong);
  line-height: 1.04;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  cursor: text;
}

.title-input {
  margin: 0 0 0.15rem;
  width: min(100%, 760px);
  border: 0;
  border-radius: 6px;
  background: color-mix(in srgb, var(--fox-chip) 70%, transparent 30%);
  padding: 0.12rem 0.3rem;
  font-family: inherit;
  font-size: 2.7rem;
  font-weight: 700;
  color: var(--fox-text-strong);
  line-height: 1.04;
}

.title-input:focus {
  outline: 1px solid color-mix(in srgb, var(--fox-primary) 45%, transparent 55%);
}

.meta-line {
  margin: 0;
  color: var(--fox-text-muted);
  font-size: 1.02rem;
  display: flex;
  align-items: center;
  gap: 0.4rem;
  flex-wrap: wrap;
}

.dot {
  opacity: 0.65;
}

.meta-tag-pill {
  display: inline-flex;
  align-items: center;
  border-radius: 6px;
  padding: 0.08rem 0.4rem;
  font-size: 0.88rem;
  line-height: 1.2;
}

.tag-area-trigger {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  flex-wrap: wrap;
  border: 0;
  background: transparent;
  padding: 0;
  margin: 0;
  cursor: pointer;
}

.empty-tag-label {
  color: var(--fox-text-muted);
  font-size: 0.9rem;
}

.tag-area-trigger:hover .meta-tag-pill,
.tag-area-trigger:hover .empty-tag-label {
  filter: brightness(1.08);
}

.tag-menu {
  width: min(345px, 72vw);
  border: 1px solid var(--fox-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--fox-surface) 90%, black 10%);
  padding: 0.65rem;
  box-shadow: 0 14px 30px rgba(0, 0, 0, 0.35);
}

.tag-menu-label {
  margin: 0 0 0.45rem;
  font-size: 1rem;
  color: var(--fox-text-strong);
  font-weight: 600;
}

.tag-filter-input {
  margin-bottom: 0.55rem;
}

.tag-options {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  max-height: 224px;
  overflow: auto;
}

.tag-option-row {
  width: 100%;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--fox-text-body);
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 0.45rem;
  padding: 0.22rem 0.28rem;
  text-align: left;
  cursor: pointer;
}

.tag-option-row:hover {
  background: var(--fox-chip);
}

.create-row {
  grid-template-columns: auto 1fr;
}

.drag-handle {
  color: var(--fox-text-muted);
  font-size: 0.82rem;
  letter-spacing: -0.04em;
}

.tag-pill {
  display: inline-flex;
  align-items: center;
  border-radius: 6px;
  padding: 0.1rem 0.42rem;
  width: fit-content;
  font-size: 0.9rem;
}

.tone-amber {
  background: rgba(255, 176, 0, 0.28);
  color: #ffba3d;
}

.tone-orange {
  background: rgba(255, 94, 0, 0.26);
  color: #ff7f3e;
}

.tone-gray {
  background: rgba(190, 190, 190, 0.2);
  color: #cbcbcb;
}

.tone-cyan {
  background: rgba(0, 199, 192, 0.25);
  color: #43dfda;
}

.tone-magenta {
  background: rgba(166, 52, 255, 0.24);
  color: #cf87ff;
}

.block-stack {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  position: relative;
  margin-top: 1rem;
}

.block-shell {
  position: relative;
  margin: 0;
  border-radius: 10px;
  min-height: 2.6rem;
  display: flex;
  flex-wrap: nowrap;
}

.block-shell::before,
.block-shell::after {
  content: "";
  position: absolute;
  left: 3.5rem;
  right: 0.55rem;
  height: 2px;
  border-radius: 999px;
  background: color-mix(in srgb, #ffffff 90%, var(--fox-primary) 10%);
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.2), 0 0 14px rgba(255, 255, 255, 0.32);
  opacity: 0;
  transform: scaleX(0.92);
  transform-origin: left center;
  transition: opacity 120ms ease, transform 150ms cubic-bezier(0.2, 0.8, 0.2, 1);
  pointer-events: none;
  z-index: 4;
}

.block-shell::before {
  top: -3px;
}

.block-shell::after {
  bottom: -3px;
}

.block-shell.is-adding-top::before,
.block-shell.is-adding-bottom::after {
  opacity: 1;
  transform: scaleX(1);
}

.block-shell+.block-shell {
  margin-top: 0;
}

.block-hover-actions {
  left: 0.5rem;
  top: 0.2rem;
  display: flex;
  flex-direction: row;
  gap: 0.36rem;
  opacity: 0;
  transform: translateY(-1px);
  transition: opacity 140ms ease, transform 140ms ease;
  pointer-events: none;
  z-index: 3;
  margin-left: 0.5rem;
}

.block-shell:hover .block-hover-actions,
.block-shell:focus-within .block-hover-actions {
  opacity: 1;
  transform: translateY(0);
}

.block-shell.selected .block-hover-actions {
  opacity: 1;
}

.round-action-btn {
  border: 1px solid color-mix(in srgb, var(--fox-border) 84%, transparent 16%);
  background: color-mix(in srgb, var(--fox-surface) 90%, black 10%);
  color: var(--fox-text-body);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  pointer-events: auto;
}

.round-action-btn.add-btn {
  width: 26px;
  height: 26px;
  border-radius: 100%;
}

.round-action-btn.menu-btn {
  width: 18px;
  height: 30px;
  border-radius: 30%;
  cursor: grab;
}

.round-action-btn.menu-btn:active {
  cursor: grabbing;
}

.round-action-btn:hover {
  color: var(--fox-text-strong);
  border-color: color-mix(in srgb, var(--fox-text-muted) 62%, transparent 38%);
  background: color-mix(in srgb, var(--fox-chip) 66%, transparent 34%);
}

.menu-anchor {
  display: inline-block;
  width: 1px;
  height: 1px;
}

.slash-menu {
  width: min(338px, 72vw);
  border: 1px solid var(--fox-border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--fox-surface) 93%, black 7%);
  padding: 0.68rem;
  box-shadow: 0 15px 28px rgba(0, 0, 0, 0.34);
}

.slash-menu-title {
  margin: 0 0 0.45rem;
  font-size: 0.92rem;
  font-weight: 600;
  color: var(--fox-text-strong);
}

.slash-search {
  margin-bottom: 0.48rem;
}

.slash-options {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.slash-option {
  width: 100%;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--fox-text-body);
  text-align: left;
  padding: 0.42rem 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.08rem;
  cursor: pointer;
}

.slash-option:hover {
  background: var(--fox-chip);
}

.slash-option-label {
  font-family: "Iosevka", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  color: var(--fox-text-strong);
}

.slash-option-desc {
  color: var(--fox-text-muted);
  font-size: 0.82rem;
}

.action-menu {
  width: min(330px, 72vw);
  border: 1px solid var(--fox-border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--fox-surface) 93%, black 7%);
  padding: 0.68rem;
  box-shadow: 0 16px 30px rgba(0, 0, 0, 0.36);
}

.action-search {
  margin-bottom: 0.5rem;
}

.action-group {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.action-row {
  width: 100%;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--fox-text-body);
  padding: 0.45rem 0.55rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
}

.action-row:hover {
  background: var(--fox-chip);
}

.action-row.danger:hover {
  background: rgba(212, 73, 73, 0.2);
  color: #ffb9b9;
}

.action-hotkey {
  color: var(--fox-text-muted);
  font-size: 0.82rem;
}

.empty {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: flex-start;
  gap: 0.2rem;
}

.empty h2 {
  font-size: 1.25rem;
}

.empty p {
  margin: 0;
  color: var(--fox-text-muted);
}

.block-shell.selected {
  outline: 1px solid color-mix(in srgb, var(--fox-primary) 35%, transparent 65%);
  outline-offset: -1px;
  background: color-mix(in srgb, var(--fox-chip) 20%, transparent 80%);
}

.marquee-selection {
  position: absolute;
  border: 1px solid #3aa5ff;
  background: color-mix(in srgb, #2f8bff 26%, transparent 74%);
  border-radius: 6px;
  pointer-events: none;
  z-index: 8;
}

@media (hover: none) and (pointer: coarse) {
  .block-hover-actions {
    position: static;
    opacity: 1;
    transform: none;
    flex-direction: row;
    margin-bottom: 0.3rem;
  }
}
</style>
