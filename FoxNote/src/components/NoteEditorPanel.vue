<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import {
  convertMarkdownTextToTypst,
  loadNoteAttachment,
  saveNoteAttachment,
  saveNoteImageAttachment,
} from "../lib/noteApi";
import UnsupportedBlockCard from "./blocks/UnsupportedBlockCard.vue";
import { createDefaultBlockForType, getBlockPlugin, listBlockPlugins } from "../editor/plugins/registry";
import { blockPreviewText, cloneBlocks } from "../editor/utils";
import type { NoteBlock, NoteDocument, NoteRecord } from "../types/note";
import type { BlockRenderContext } from "../editor/plugins/types";
import { splitTypstContentIntoBlocks } from "../editor/typstHeadingSplit";
import { shouldSkipMarqueeStart } from "../editor/interactionTargets";

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
const markdownImportDialogOpen = ref(false);
const markdownImportTargetIndex = ref<number | null>(null);
const markdownImportInput = ref("");
const markdownImportBusy = ref(false);
const editingBlockIndex = ref<number | null>(null);
const isHydratingForm = ref(false);
const titleEditing = ref(false);
const titleInputRef = ref<HTMLInputElement | null>(null);
const titleDisplayRef = ref<HTMLElement | null>(null);
const tagAreaTriggerRef = ref<HTMLElement | null>(null);
const editorPaneRef = ref<HTMLElement | null>(null);
const blockStackRef = ref<HTMLElement | null>(null);
const blockElements = ref<(HTMLElement | null)[]>([]);
const findInputRef = ref<HTMLInputElement | null>(null);
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
const findBarOpen = ref(false);
const findQuery = ref("");
const currentFindMatchIndex = ref(0);

type NoteFindMatch =
  | { kind: "title" }
  | { kind: "tags" }
  | { kind: "block"; index: number };

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

const findMatches = computed<NoteFindMatch[]>(() => {
  const query = findQuery.value.trim().toLowerCase();
  if (!query) {
    return [];
  }

  const matches: NoteFindMatch[] = [];
  if ((form.title || "").toLowerCase().includes(query)) {
    matches.push({ kind: "title" });
  }

  if (form.tags.some((tag) => tag.toLowerCase().includes(query))) {
    matches.push({ kind: "tags" });
  }

  for (let index = 0; index < form.content.length; index += 1) {
    if (isBlockHidden(index)) {
      continue;
    }

    const block = form.content[index];
    if (!block) {
      continue;
    }

    const searchable = blockPreviewText(block).trim().toLowerCase();
    if (searchable && searchable.includes(query)) {
      matches.push({ kind: "block", index });
    }
  }

  return matches;
});

const currentFindMatch = computed<NoteFindMatch | null>(() => {
  if (findMatches.value.length === 0) {
    return null;
  }
  const index = Math.max(0, Math.min(currentFindMatchIndex.value, findMatches.value.length - 1));
  return findMatches.value[index] ?? null;
});

const findStatusLabel = computed(() => {
  const total = findMatches.value.length;
  if (!findQuery.value.trim()) {
    return "Type to search this note";
  }
  if (total === 0) {
    return "No matches";
  }
  return `${Math.min(currentFindMatchIndex.value + 1, total)} / ${total}`;
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

function updateBlockSize(index: number, nextWidth: number, nextHeight: number) {
  const block = form.content[index];
  if (!block) {
    return;
  }

  const width = Math.max(140, Math.round(Number(nextWidth) || 0));
  const height = Math.max(120, Math.round(Number(nextHeight) || 0));
  if (block.width === width && block.height === height) {
    return;
  }

  updateBlock(index, {
    ...block,
    width,
    height,
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
    updateSize: (nextWidth, nextHeight) => updateBlockSize(index, nextWidth, nextHeight),
    updateLevel: (nextLevel) => updateBlockLevel(index, nextLevel),
    updateFolded: (nextFolded) => updateBlockFolded(index, nextFolded),
    updateSummary: (nextSummary) => updateBlockSummary(index, nextSummary),
  };
}

function splitTypstHeadingBlock(index: number, content: string): boolean {
  const block = form.content[index];
  if (!block || block.type !== "typst") {
    return false;
  }

  const replacement = splitTypstContentIntoBlocks(content);
  if (!replacement) {
    return false;
  }

  const blocks = cloneBlocks(form.content);
  blocks.splice(index, 1, ...replacement);

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

function openMarkdownImportDialog(index: number) {
  logMarkdownImport("open dialog", {
    index,
    noteId: props.note?.id ?? null,
    blockCount: form.content.length,
  });
  markdownImportTargetIndex.value = index;
  markdownImportInput.value = "";
  markdownImportDialogOpen.value = true;
  closeSlashMenu();
  void nextTick(() => {
    const textarea = document.querySelector<HTMLTextAreaElement>(".markdown-import-input textarea");
    textarea?.focus();
  });
}

function closeMarkdownImportDialog() {
  if (markdownImportBusy.value) {
    logMarkdownImport("close dialog ignored because busy", {
      targetIndex: markdownImportTargetIndex.value,
    });
    return;
  }
  logMarkdownImport("close dialog", {
    targetIndex: markdownImportTargetIndex.value,
    inputLength: markdownImportInput.value.length,
  });
  markdownImportDialogOpen.value = false;
  markdownImportInput.value = "";
  markdownImportTargetIndex.value = null;
}

async function confirmMarkdownImport() {
  logMarkdownImport("confirm clicked", {
    hasNote: Boolean(props.note),
    busy: markdownImportBusy.value,
    inputLength: markdownImportInput.value.length,
    targetIndex: markdownImportTargetIndex.value,
  });

  if (!props.note || markdownImportBusy.value) {
    logMarkdownImport("confirm aborted by guard", {
      hasNote: Boolean(props.note),
      busy: markdownImportBusy.value,
    });
    return;
  }

  const markdown = markdownImportInput.value.trim();
  if (!markdown) {
    logMarkdownImport("confirm aborted: empty markdown", {
      rawLength: markdownImportInput.value.length,
    });
    return;
  }

  markdownImportBusy.value = true;
  logMarkdownImport("conversion started", {
    markdownLength: markdown.length,
  });
  try {
    const typstSource = await convertMarkdownTextToTypst(markdown);
    const content = typstSource.trim();
    logMarkdownImport("conversion finished", {
      typstLength: typstSource.length,
      trimmedLength: content.length,
      typstPreview: typstSource.slice(0, 220),
    });
    if (!content) {
      logMarkdownImport("conversion result empty, closing dialog");
      closeMarkdownImportDialog();
      return;
    }

    const blocks = cloneBlocks(form.content);
    const baseIndex = markdownImportTargetIndex.value ?? form.content.length - 1;
    const insertIndex = Math.max(0, Math.min(blocks.length, baseIndex + 1));
    logMarkdownImport("inserting typst block", {
      baseIndex,
      insertIndex,
      blocksBefore: blocks.length,
    });

    blocks.splice(insertIndex, 0, {
      type: "typst",
      content,
    });

    form.content = blocks;
    logMarkdownImport("insert success", {
      blocksAfter: form.content.length,
      selectedIndex: insertIndex,
    });
    selectOnly(insertIndex);
    setEditingBlock(insertIndex);
    closeActionMenu();
    closeMarkdownImportDialog();
  } catch (reason) {
    console.error("[MarkdownImport] failed to import markdown", {
      reason,
      markdownLength: markdown.length,
      targetIndex: markdownImportTargetIndex.value,
    });
  } finally {
    markdownImportBusy.value = false;
    logMarkdownImport("confirm finished", {
      dialogOpen: markdownImportDialogOpen.value,
      busy: markdownImportBusy.value,
    });
  }
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
  closeMarkdownImportDialog();
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

function focusFindInput(select = false) {
  void nextTick(() => {
    findInputRef.value?.focus();
    if (select) {
      findInputRef.value?.select();
    }
  });
}

function openFindBar(select = false) {
  findBarOpen.value = true;
  focusFindInput(select);
}

function closeFindBar() {
  findBarOpen.value = false;
  findQuery.value = "";
  currentFindMatchIndex.value = 0;
}

function scrollCurrentFindMatchIntoView() {
  const match = currentFindMatch.value;
  if (!match) {
    return;
  }

  if (match.kind === "title") {
    (titleEditing.value ? titleInputRef.value : titleDisplayRef.value)?.scrollIntoView({
      block: "center",
      behavior: "smooth",
    });
    return;
  }

  if (match.kind === "tags") {
    tagAreaTriggerRef.value?.scrollIntoView({ block: "center", behavior: "smooth" });
    return;
  }

  const element = blockElements.value[match.index];
  element?.scrollIntoView({ block: "center", behavior: "smooth" });
  selectOnly(match.index);
}

function stepFindMatch(direction: 1 | -1) {
  const total = findMatches.value.length;
  if (total === 0) {
    return;
  }
  currentFindMatchIndex.value = (currentFindMatchIndex.value + direction + total) % total;
  scrollCurrentFindMatchIntoView();
}

function isFindMatchTitle(): boolean {
  return findMatches.value.some((match) => match.kind === "title");
}

function isCurrentFindTitle(): boolean {
  return currentFindMatch.value?.kind === "title";
}

function isFindMatchTags(): boolean {
  return findMatches.value.some((match) => match.kind === "tags");
}

function isCurrentFindTags(): boolean {
  return currentFindMatch.value?.kind === "tags";
}

function isFindMatchBlock(index: number): boolean {
  return findMatches.value.some((match) => match.kind === "block" && match.index === index);
}

function isCurrentFindBlock(index: number): boolean {
  return currentFindMatch.value?.kind === "block" && currentFindMatch.value.index === index;
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

   return Boolean(target.closest(".v-field, .v-input, .v-textarea, .ProseMirror"));
}

function isTextEditingContext(target: EventTarget | null): boolean {
  if (isTextEntryTarget(target)) {
    return true;
  }

  if (typeof document === "undefined") {
    return false;
  }

  return isTextEntryTarget(document.activeElement);
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
  const key = event.key.toLowerCase();

  if ((event.metaKey || event.ctrlKey) && key === "f") {
    event.preventDefault();
    openFindBar(true);
    return;
  }

  if (key === "escape" && findBarOpen.value) {
    event.preventDefault();
    closeFindBar();
    return;
  }

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

  if (isTextEditingContext(event.target)) {
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

  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "c") {
    event.preventDefault();
    copySelectedBlocks();
    return;
  }

  if (event.key === "Escape") {
    if (markdownImportDialogOpen.value) {
      event.preventDefault();
      closeMarkdownImportDialog();
      return;
    }
    clearSelection();
  }
}

const FOXNOTE_BLOCKS_MIME = "application/x-foxnote-blocks";
const FOXNOTE_BLOCKS_TEXT_PREFIX = "FOXNOTE_BLOCKS::";
const DEBUG_BLOCK_CLIPBOARD = true;
const DEBUG_MARKDOWN_IMPORT = true;

function logMarkdownImport(message: string, details?: Record<string, unknown>) {
  if (!DEBUG_MARKDOWN_IMPORT) {
    return;
  }
  console.debug(`[MarkdownImport] ${message}`, details ?? {});
}

function logBlockClipboard(message: string, details?: Record<string, unknown>) {
  if (!DEBUG_BLOCK_CLIPBOARD) {
    return;
  }
  console.debug(`[BlockClipboard] ${message}`, details ?? {});
}

function collectSelectedBlocksForClipboard(): NoteBlock[] {
  const sourceIndices = [...selectedBlocks.value].sort((a, b) => a - b);
  return sourceIndices
    .map((index) => form.content[index])
    .filter((block): block is NoteBlock => Boolean(block))
    .map((block) => ({ ...block }));
}

function buildClipboardPayload(blocks: NoteBlock[]): { json: string; plainText: string } {
  const json = JSON.stringify({
    sourceNoteId: props.note?.id ?? null,
    blocks,
  });
  const plainText = `${FOXNOTE_BLOCKS_TEXT_PREFIX}${json}`;
  return { json, plainText };
}

function copySelectedBlocks() {
  if (selectedBlocks.value.length === 0) {
    logBlockClipboard("copy aborted: no selected blocks");
    return;
  }

  const copied = collectSelectedBlocksForClipboard();

  if (copied.length === 0) {
    logBlockClipboard("copy aborted: selected blocks resolved empty", {
      selectedCount: selectedBlocks.value.length,
    });
    return;
  }

  const payload = buildClipboardPayload(copied);
  logBlockClipboard("copy requested via keybinding", {
    copiedCount: copied.length,
    selectedIndices: [...selectedBlocks.value],
    jsonLength: payload.json.length,
    hasNavigatorClipboard: typeof navigator !== "undefined" && Boolean(navigator.clipboard),
  });

  if (typeof navigator === "undefined" || !navigator.clipboard || typeof ClipboardItem === "undefined") {
    logBlockClipboard("copy keybinding fallback unavailable: clipboard APIs missing");
    return;
  }

  const blocksBlob = new Blob([payload.json], { type: FOXNOTE_BLOCKS_MIME });
  const textBlob = new Blob([payload.plainText], { type: "text/plain" });

  void navigator.clipboard.write([
    new ClipboardItem({ [FOXNOTE_BLOCKS_MIME]: blocksBlob, "text/plain": textBlob }),
  ]).then(() => {
    logBlockClipboard("copy keybinding write success", { copiedCount: copied.length });
  }).catch((reason) => {
    console.error("[BlockClipboard] copy keybinding write failed", reason);
  });
}

function onWindowCopy(event: ClipboardEvent) {
  if (!props.note) {
    return;
  }

  if (selectedBlocks.value.length === 0) {
    return;
  }

  if (isTextEntryTarget(event.target)) {
    logBlockClipboard("native copy ignored: text entry target", {
      targetType: resolveTargetElement(event.target)?.tagName,
    });
    return;
  }

  const copied = collectSelectedBlocksForClipboard();
  if (copied.length === 0) {
    logBlockClipboard("native copy aborted: selected blocks resolved empty");
    return;
  }

  const payload = buildClipboardPayload(copied);
  event.preventDefault();
  event.clipboardData?.setData(FOXNOTE_BLOCKS_MIME, payload.json);
  event.clipboardData?.setData("text/plain", payload.plainText);
  logBlockClipboard("native copy set clipboard data", {
    copiedCount: copied.length,
    selectedIndices: [...selectedBlocks.value],
    jsonLength: payload.json.length,
  });
}

function tryParseFoxnoteBlocks(raw: string): { sourceNoteId: string | null; blocks: NoteBlock[] } | null {
  if (!raw) {
    return null;
  }

  const payload = raw.startsWith(FOXNOTE_BLOCKS_TEXT_PREFIX)
    ? raw.slice(FOXNOTE_BLOCKS_TEXT_PREFIX.length)
    : raw;

  const parsed = JSON.parse(payload);

  if (Array.isArray(parsed) && parsed.length > 0 && parsed[0]?.type) {
    return {
      sourceNoteId: null,
      blocks: parsed.map((block: NoteBlock) => ({ ...block })),
    };
  }

  const parsedBlocks = parsed?.blocks;
  if (!Array.isArray(parsedBlocks) || parsedBlocks.length === 0 || !parsedBlocks[0]?.type) {
    return null;
  }

  return {
    sourceNoteId: typeof parsed?.sourceNoteId === "string" ? parsed.sourceNoteId : null,
    blocks: parsedBlocks.map((block: NoteBlock) => ({ ...block })),
  };
}

function extensionFromPath(path: string): string | undefined {
  const slashIndex = path.lastIndexOf("/");
  const fileName = slashIndex >= 0 ? path.slice(slashIndex + 1) : path;
  const dotIndex = fileName.lastIndexOf(".");
  if (dotIndex < 0 || dotIndex === fileName.length - 1) {
    return undefined;
  }
  return fileName.slice(dotIndex + 1).toLowerCase();
}

async function cloneBlockAttachmentsForTarget(
  sourceNoteId: string | null,
  targetNoteId: string,
  blocks: NoteBlock[],
): Promise<NoteBlock[]> {
  if (!sourceNoteId || sourceNoteId === targetNoteId) {
    return blocks.map((block) => ({ ...block }));
  }

  logBlockClipboard("cross-note paste detected, cloning attachments", {
    sourceNoteId,
    targetNoteId,
    blockCount: blocks.length,
  });

  const result: NoteBlock[] = [];
  for (const block of blocks) {
    if (!block.path) {
      result.push({ ...block });
      continue;
    }

    try {
      const payload = await loadNoteAttachment(sourceNoteId, block.path);
      const bytes = payload.bytes ?? [];
      const mimeType = payload.mimeType || "application/octet-stream";
      const extension = extensionFromPath(block.path);
      const nextPath = await saveNoteAttachment(
        targetNoteId,
        mimeType,
        bytes,
        block.type || "attachment",
        extension,
      );
      result.push({ ...block, path: nextPath });
      logBlockClipboard("attachment cloned", {
        blockType: block.type,
        oldPath: block.path,
        newPath: nextPath,
        mimeType,
      });
    } catch (reason) {
      console.error("[BlockClipboard] failed to clone block attachment", {
        sourceNoteId,
        targetNoteId,
        path: block.path,
        reason,
      });
      result.push({ ...block });
    }
  }

  return result;
}

function insertTypstBlockFromPaste(text: string) {
  const pasted = text.replace(/\r\n/g, "\n");
  if (!pasted.trim()) {
    return;
  }

  const blocks = cloneBlocks(form.content);
  const baseIndex = selectedBlocks.value[selectedBlocks.value.length - 1] ?? form.content.length - 1;
  const insertIndex = Math.max(0, Math.min(blocks.length, baseIndex + 1));

  const inserted = splitTypstContentIntoBlocks(pasted);
  if (inserted) {
    blocks.splice(insertIndex, 0, ...inserted);
    form.content = blocks;

    const focusIndex = insertIndex + inserted.length - 1;
    selectOnly(focusIndex);
    setEditingBlock(focusIndex);
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
    logBlockClipboard("paste ignored: text entry target", {
      targetType: resolveTargetElement(event.target)?.tagName,
    });
    return;
  }

  logBlockClipboard("paste event received", {
    clipboardTypes: [...(event.clipboardData?.types ?? [])],
    itemTypes: [...(event.clipboardData?.items ?? [])].map((item) => item.type),
    selectedCount: selectedBlocks.value.length,
  });

  const foxnoteItem = [...(event.clipboardData?.items ?? [])].find(
    (item) => item.type === FOXNOTE_BLOCKS_MIME,
  );
  if (foxnoteItem) {
    event.preventDefault();
    try {
      const blob = foxnoteItem.getAsFile();
      if (blob) {
        const text = await blob.text();
        const parsed = tryParseFoxnoteBlocks(text);
        if (parsed) {
          const blocks = cloneBlocks(form.content);
          const baseIndex = selectedBlocks.value[selectedBlocks.value.length - 1] ?? form.content.length - 1;
          const insertIndex = Math.max(0, Math.min(blocks.length, baseIndex + 1));
          const pastedBlocks = await cloneBlockAttachmentsForTarget(
            parsed.sourceNoteId,
            props.note.id,
            parsed.blocks,
          );
          blocks.splice(insertIndex, 0, ...pastedBlocks);
          form.content = blocks;
          const inserted = pastedBlocks.map((_: NoteBlock, i: number) => insertIndex + i);
          normalizeSelection(inserted);
          selectionAnchor.value = inserted[inserted.length - 1] ?? null;
          setEditingBlock(inserted[0] ?? null);
          closeSlashMenu();
          closeActionMenu();
          logBlockClipboard("paste foxnote MIME success", {
            pastedCount: pastedBlocks.length,
            insertIndex,
            sourceNoteId: parsed.sourceNoteId,
            targetNoteId: props.note.id,
          });
          return;
        }
      }
    } catch (reason) {
      console.error("failed to paste foxnote blocks", reason);
    }
    logBlockClipboard("paste foxnote MIME failed, falling back to text/image");
  }

  const pastedText = event.clipboardData?.getData("text/plain") ?? "";
  if (pastedText.startsWith(FOXNOTE_BLOCKS_TEXT_PREFIX)) {
    event.preventDefault();
    try {
      const parsed = tryParseFoxnoteBlocks(pastedText);
      if (parsed) {
        const blocks = cloneBlocks(form.content);
        const baseIndex = selectedBlocks.value[selectedBlocks.value.length - 1] ?? form.content.length - 1;
        const insertIndex = Math.max(0, Math.min(blocks.length, baseIndex + 1));
        const pastedBlocks = await cloneBlockAttachmentsForTarget(
          parsed.sourceNoteId,
          props.note.id,
          parsed.blocks,
        );
        blocks.splice(insertIndex, 0, ...pastedBlocks);
        form.content = blocks;
        const inserted = pastedBlocks.map((_: NoteBlock, i: number) => insertIndex + i);
        normalizeSelection(inserted);
        selectionAnchor.value = inserted[inserted.length - 1] ?? null;
        setEditingBlock(inserted[0] ?? null);
        closeSlashMenu();
        closeActionMenu();
        logBlockClipboard("paste text-prefix success", {
          pastedCount: pastedBlocks.length,
          insertIndex,
          sourceNoteId: parsed.sourceNoteId,
          targetNoteId: props.note.id,
        });
      }
      return;
    } catch (reason) {
      console.error("failed to paste foxnote blocks from text prefix", reason);
    }
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

  if (!pastedText.trim()) {
    logBlockClipboard("paste ignored: empty plain text and no image/foxnote blocks");
    return;
  }

  event.preventDefault();
  logBlockClipboard("paste plain text fallback to typst block", {
    textLength: pastedText.length,
  });
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

  if (shouldSkipMarqueeStart(targetEl)) {
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
      closeFindBar();
      closeMarkdownImportDialog();
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
    closeFindBar();
    closeMarkdownImportDialog();
    isHydratingForm.value = false;
  },
  { immediate: true },
);

watch(findQuery, () => {
  currentFindMatchIndex.value = 0;
  if (findQuery.value.trim()) {
    void nextTick(() => {
      scrollCurrentFindMatchIntoView();
    });
  }
});

watch(
  () => findMatches.value.length,
  (length) => {
    if (length <= 0) {
      currentFindMatchIndex.value = 0;
      return;
    }
    if (currentFindMatchIndex.value >= length) {
      currentFindMatchIndex.value = 0;
    }
  },
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
  window.addEventListener("copy", onWindowCopy);
  window.addEventListener("paste", onWindowPaste);
  window.addEventListener("mousemove", onWindowMouseMove);
  window.addEventListener("mouseup", stopMarquee);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onMenuHandlePointerMove);
  window.removeEventListener("pointerup", onMenuHandlePointerUp);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("copy", onWindowCopy);
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
  <div v-if="findBarOpen" class="note-find-bar" @click.stop>
    <input ref="findInputRef" v-model="findQuery" type="text" class="note-find-input" placeholder="Find in note"
      @keydown.enter.prevent="stepFindMatch($event.shiftKey ? -1 : 1)" @keydown.esc.prevent="closeFindBar" />
    <span class="note-find-status">{{ findStatusLabel }}</span>
    <div class="note-find-actions">
      <button type="button" class="note-find-icon-btn" :disabled="findMatches.length === 0" title="Previous match"
        @click="stepFindMatch(-1)">
        <v-icon icon="mdi-chevron-up" size="16" />
      </button>
      <button type="button" class="note-find-icon-btn" :disabled="findMatches.length === 0" title="Next match"
        @click="stepFindMatch(1)">
        <v-icon icon="mdi-chevron-down" size="16" />
      </button>
    </div>
    <button type="button" class="note-find-close-btn" title="Close find" @click="closeFindBar">
      <v-icon icon="mdi-close" size="15" />
    </button>
  </div>

  <section ref="editorPaneRef" class="editor-pane" :class="{ 'marquee-active': marquee.pending || marquee.active }"
    v-if="note" @click.self="exitBlockEditMode" @mousedown.capture="onEditorPaneMouseDown"
    @click.capture="onEditorPaneClickCapture">

    <header class="note-top">
      <div class="meta-wrap">
        <button v-if="!titleEditing" ref="titleDisplayRef" type="button" class="title-display"
          :class="{ 'find-hit': isFindMatchTitle(), 'find-current-hit': isCurrentFindTitle() }" @click="startTitleEdit">
          {{ form.title || "Untitled" }}
        </button>
        <input v-else ref="titleInputRef" v-model="form.title" class="title-input"
          :class="{ 'find-hit': isFindMatchTitle(), 'find-current-hit': isCurrentFindTitle() }" type="text"
          @blur="finishTitleEdit" @keydown.enter.prevent="finishTitleEdit" @keydown.esc.prevent="finishTitleEdit" />
        <p class="meta-line">
          <span>{{ noteTypeLabel }}</span>
          <span class="dot">•</span>
          <v-menu v-model="tagMenuOpen" location="bottom start" :close-on-content-click="false">
            <template #activator="{ props: menuProps }">
              <button ref="tagAreaTriggerRef" type="button" class="tag-area-trigger"
                :class="{ 'find-hit': isFindMatchTags(), 'find-current-hit': isCurrentFindTags() }" v-bind="menuProps">
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
        v-show="!isBlockHidden(index)" :ref="(el) => setBlockElement(index, el)" class="block-shell" :class="{
          selected: isBlockSelected(index),
          'is-adding-top': isDropTargetTop(index),
          'is-adding-bottom': isDropTargetBottom(index),
          'find-hit': isFindMatchBlock(index),
          'find-current-hit': isCurrentFindBlock(index),
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
                <button type="button" class="slash-option" @click="openMarkdownImportDialog(index)">
                  <span class="slash-option-label">/import-md</span>
                  <span class="slash-option-desc">Import Markdown and convert to Typst block</span>
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

    <v-dialog v-model="markdownImportDialogOpen" max-width="760">
      <v-card>
        <v-card-title>Import Markdown</v-card-title>
        <v-card-text>
          <v-textarea v-model="markdownImportInput" class="markdown-import-input" rows="12" auto-grow variant="outlined"
            hide-details placeholder="Paste Markdown here..." />
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" :disabled="markdownImportBusy" @click="closeMarkdownImportDialog">Cancel</v-btn>
          <v-btn color="primary" variant="flat" :loading="markdownImportBusy" :disabled="!markdownImportInput.trim()"
            @click="confirmMarkdownImport">
            Import as Typst block
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

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
  width: min(100%, 80rem);
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

.note-find-bar {
  position: absolute;
  top: 3.8rem;
  right: 0.55rem;
  z-index: 14;
  width: min(330px, calc(100% - 1.1rem));
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto auto;
  gap: 0.28rem;
  align-items: center;
  padding: 0.34rem 0.38rem;
  border: 1px solid color-mix(in srgb, var(--fox-border) 82%, transparent 18%);
  border-radius: 12px;
  background: color-mix(in srgb, #171c26 94%, var(--fox-surface) 6%);
  box-shadow: 0 8px 18px rgba(0, 0, 0, 0.24);
}

.note-find-input {
  width: 100%;
  min-width: 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--fox-text-strong);
  padding: 0.34rem 0.48rem;
  font-size: 0.92rem;
}

.note-find-input:focus {
  outline: 0;
}

.note-find-status {
  color: var(--fox-text-muted);
  font-size: 0.77rem;
  white-space: nowrap;
  justify-self: center;
  min-width: 3.1rem;
  text-align: center;
}

.note-find-actions {
  display: inline-flex;
  align-items: center;
  gap: 0;
  border: 1px solid color-mix(in srgb, var(--fox-border) 78%, transparent 22%);
  border-radius: 8px;
  overflow: hidden;
  background: color-mix(in srgb, var(--fox-surface) 92%, black 8%);
}

.note-find-icon-btn,
.note-find-close-btn {
  border: 0;
  background: transparent;
  color: var(--fox-text-body);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  width: 28px;
  height: 28px;
  padding: 0;
}

.note-find-icon-btn+.note-find-icon-btn {
  border-left: 1px solid color-mix(in srgb, var(--fox-border) 78%, transparent 22%);
}

.note-find-icon-btn:hover:not(:disabled),
.note-find-close-btn:hover {
  background: color-mix(in srgb, var(--fox-chip) 70%, transparent 30%);
}

.note-find-icon-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.note-find-close-btn {
  color: var(--fox-text-muted);
  border-radius: 8px;
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

.find-hit {
  box-shadow: 0 0 0 1px color-mix(in srgb, #f0c75e 35%, transparent 65%);
}

.find-current-hit {
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--fox-primary) 55%, transparent 45%);
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
  margin-right: 3.2rem;
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
  overflow: auto;
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

@media (max-width: 760px) {
  .editor-pane {
    width: 100%;
    padding: 0.35rem 0 0.65rem;
  }

  .meta-wrap {
    padding-left: 0.2rem;
  }

  .title-display,
  .title-input {
    font-size: clamp(1.6rem, 7.2vw, 2.15rem);
    line-height: 1.12;
    white-space: normal;
    overflow-wrap: anywhere;
  }

  .meta-line {
    font-size: 0.9rem;
    gap: 0.28rem;
  }

  .block-stack {
    margin-top: 0.7rem;
    gap: 0.28rem;
  }

  .block-shell {
    margin-right: 0.2rem;
  }

  .block-hover-actions {
    margin-left: 0.18rem;
  }

  .round-action-btn.add-btn {
    width: 24px;
    height: 24px;
  }

  .round-action-btn.menu-btn {
    width: 18px;
    height: 26px;
  }

  .tag-menu,
  .slash-menu,
  .action-menu {
    width: min(100vw - 1.1rem, 360px);
  }

  .note-find-bar {
    position: static;
    width: auto;
    margin: 0.8rem 0 0;
    grid-template-columns: 1fr auto auto;
    box-shadow: 0 12px 24px rgba(0, 0, 0, 0.24);
  }

  .note-find-input {
    grid-column: 1 / -1;
  }

  .note-find-status {
    justify-self: start;
    min-width: 0;
  }
}
</style>
