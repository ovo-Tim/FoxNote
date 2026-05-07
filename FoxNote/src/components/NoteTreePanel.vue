<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import type { FolderEntry, NoteSummary } from "../types/note";

type TreeRow =
  | {
      kind: "folder";
      path: string;
      name: string;
      depth: number;
      hasChildren: boolean;
    }
  | {
      kind: "note";
      id: string;
      title: string;
      folder: string;
      depth: number;
    };

const props = defineProps<{
  folders: FolderEntry[];
  notes: NoteSummary[];
  selectedNoteId?: string;
  loading?: boolean;
  showEmptyFolders?: boolean;
}>();

const emit = defineEmits<{
  selectNote: [id: string];
  createNoteInFolder: [folderPath: string];
  createFolderInFolder: [folderPath: string];
  renameFolderInTree: [folderPath: string, folderName: string];
  renameNoteInTree: [noteId: string, noteTitle: string];
  deleteFolderInTree: [folderPath: string, folderName: string];
  deleteNoteInTree: [noteId: string, noteTitle: string];
  deleteTreeSelection: [payload: {
    noteIds: string[];
    noteTitles: string[];
    folderPaths: string[];
    folderNames: string[];
  }];
}>();

const expanded = ref<Record<string, boolean>>({});
const treePanelRef = ref<HTMLElement | null>(null);
const treeScrollRef = ref<HTMLElement | null>(null);
const selectedRows = ref<string[]>([]);
const DEBUG_TREE_MARQUEE = false;
const debugInfo = reactive({
  lastEvent: "idle",
  startX: 0,
  startY: 0,
  currentX: 0,
  currentY: 0,
  selectionTop: 0,
  selectionBottom: 0,
  rowsFound: 0,
  rowsHit: 0,
  latestNoteId: "",
  selectedCount: 0,
  selectedPreview: "",
  targetTag: "",
  targetClasses: "",
  scope: "none",
});
const marquee = reactive({
  pending: false,
  active: false,
  justFinished: false,
  startX: 0,
  startY: 0,
  currentX: 0,
  currentY: 0,
});
const marqueeSelectionNoteId = ref("");
const renamingFolderPath = ref("");
const renamingFolderName = ref("");
const renamingNoteId = ref("");
const renamingNoteTitle = ref("");

function debugLog(event: string, details?: Record<string, unknown>) {
  if (!DEBUG_TREE_MARQUEE) {
    return;
  }

  if (details) {
    console.debug(`[TreeMarquee] ${event}`, details);
    return;
  }

  console.debug(`[TreeMarquee] ${event}`);
}

const contextMenu = reactive({
  open: false,
  x: 0,
  y: 0,
  kind: "folder" as "folder" | "note",
  folderPath: "",
  folderName: "root",
  noteId: "",
  noteTitle: "",
});

const contextMenuStyle = computed<Record<string, string>>(() => {
  return {
    left: `${contextMenu.x}px`,
    top: `${contextMenu.y}px`,
  };
});

const selectedRowSet = computed(() => {
  return new Set(selectedRows.value);
});

const selectedTargets = computed(() => {
  const noteIds = new Set<string>();
  const folderPaths = new Set<string>();
  const notes: { id: string; title: string }[] = [];
  const folders: { path: string; name: string }[] = [];

  for (const key of selectedRows.value) {
    if (key.startsWith("note:")) {
      const id = key.slice("note:".length);
      if (!id || noteIds.has(id)) {
        continue;
      }

      const note = props.notes.find((entry) => entry.id === id);
      notes.push({
        id,
        title: note?.title || "Untitled Note",
      });
      noteIds.add(id);
      continue;
    }

    if (key.startsWith("folder:")) {
      const path = key.slice("folder:".length);
      if (!path || folderPaths.has(path)) {
        continue;
      }

      const folder = props.folders.find((entry) => entry.path === path);
      const pathSegments = path.split("/").filter(Boolean);
      const fallbackName = pathSegments[pathSegments.length - 1] || "folder";
      folders.push({
        path,
        name: folder?.name || fallbackName,
      });
      folderPaths.add(path);
    }
  }

  return {
    notes,
    folders,
  };
});

const selectedItemCount = computed(() => {
  return selectedTargets.value.notes.length + selectedTargets.value.folders.length;
});

const usingMultiSelectionContext = computed(() => {
  return selectedItemCount.value > 1;
});

const marqueeStyle = computed<Record<string, string>>(() => {
  const hiddenStyle: Record<string, string> = {
    display: "none",
    left: "0px",
    top: "0px",
    width: "0px",
    height: "0px",
  };

  const panelEl = treePanelRef.value;
  if (!marquee.active || !panelEl) {
    return hiddenStyle;
  }

  const bounds = panelEl.getBoundingClientRect();
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

const notesByFolder = computed(() => {
  const grouped = new Map<string, NoteSummary[]>();

  for (const note of props.notes) {
    const key = note.folder || "";
    if (!grouped.has(key)) {
      grouped.set(key, []);
    }
    grouped.get(key)?.push(note);
  }

  for (const entries of grouped.values()) {
    entries.sort((left, right) => left.title.localeCompare(right.title));
  }

  return grouped;
});

const folderChildren = computed(() => {
  const map = new Map<string, FolderEntry[]>();

  for (const folder of props.folders) {
    const splitAt = folder.path.lastIndexOf("/");
    const parent = splitAt >= 0 ? folder.path.slice(0, splitAt) : "";
    if (!map.has(parent)) {
      map.set(parent, []);
    }
    map.get(parent)?.push(folder);
  }

  for (const children of map.values()) {
    children.sort((left, right) => left.name.localeCompare(right.name));
  }

  return map;
});

const visibleRows = computed<TreeRow[]>(() => {
  const rows: TreeRow[] = [];
  const showEmpty = props.showEmptyFolders ?? true;

  const walk = (parentPath: string, depth: number) => {
    const children = folderChildren.value.get(parentPath) ?? [];

    for (const folder of children) {
      const childFolders = folderChildren.value.get(folder.path) ?? [];
      const notes = notesByFolder.value.get(folder.path) ?? [];
      const hasChildren = childFolders.length > 0 || notes.length > 0;

      if (!showEmpty && !hasChildren) {
        continue;
      }

      rows.push({
        kind: "folder",
        path: folder.path,
        name: folder.name,
        depth,
        hasChildren,
      });

      if (!isExpanded(folder.path)) {
        continue;
      }

      walk(folder.path, depth + 1);

      for (const note of notes) {
        rows.push({
          kind: "note",
          id: note.id,
          title: note.title,
          folder: note.folder,
          depth: depth + 1,
        });
      }
    }
  };

  walk("", 1);

  for (const note of notesByFolder.value.get("") ?? []) {
    rows.unshift({
      kind: "note",
      id: note.id,
      title: note.title,
      folder: "",
      depth: 1,
    });
  }

  return rows;
});

watch(
  () => props.folders.map((folder) => folder.path),
  (paths) => {
    const next: Record<string, boolean> = {};

    for (const path of paths) {
      next[path] = expanded.value[path] ?? true;
    }

    expanded.value = next;
  },
  { immediate: true },
);

watch(
  () => visibleRows.value.map((row) => rowKey(row)),
  (keys) => {
    const allowed = new Set(keys);
    selectedRows.value = selectedRows.value.filter((row) => allowed.has(row));
  },
);

watch(
  selectedRows,
  (rows) => {
    debugInfo.selectedCount = rows.length;
    debugInfo.selectedPreview = rows.slice(0, 6).join(", ");
    debugLog("selectedRows changed", {
      count: rows.length,
      rows: [...rows],
    });
  },
  { deep: false },
);

onMounted(() => {
  window.addEventListener("mousemove", onWindowMouseMove);
  window.addEventListener("mouseup", onWindowMouseUp);
  window.addEventListener("keydown", onWindowKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("mousemove", onWindowMouseMove);
  window.removeEventListener("mouseup", onWindowMouseUp);
  window.removeEventListener("keydown", onWindowKeydown);
  setGlobalMarqueeTextSelection(false);
});

function rowStyle(depth: number): Record<string, string> {
  return {
    paddingLeft: `${depth * 1.05 + 0.25}rem`,
  };
}

function rowKey(row: TreeRow): string {
  return row.kind === "folder" ? `folder:${row.path}` : `note:${row.id}`;
}

function isRowSelected(row: TreeRow): boolean {
  return selectedRowSet.value.has(rowKey(row));
}

function clearTreeSelection() {
  selectedRows.value = [];
  marqueeSelectionNoteId.value = "";
}

function parentFolderPath(path: string): string {
  const splitAt = path.lastIndexOf("/");
  return splitAt >= 0 ? path.slice(0, splitAt) : "";
}

function folderNameFromPath(path: string): string {
  const pathSegments = path.split("/").filter(Boolean);
  return pathSegments[pathSegments.length - 1] || "folder";
}

function folderPathWithName(path: string, nextName: string): string {
  const parent = parentFolderPath(path);
  return parent ? `${parent}/${nextName}` : nextName;
}

function selectSingleRow(row: TreeRow) {
  selectedRows.value = [rowKey(row)];
  marqueeSelectionNoteId.value = row.kind === "note" ? row.id : "";
}

function isTextEntryTarget(target: EventTarget | null): boolean {
  const targetEl = resolveTargetElement(target);
  if (!targetEl) {
    return false;
  }

  return Boolean(targetEl.closest("input, textarea, [contenteditable='true'], .v-field"));
}

function isRenamingFolderRow(row: TreeRow): boolean {
  return row.kind === "folder" && row.path === renamingFolderPath.value;
}

function isRenamingNoteRow(row: TreeRow): boolean {
  return row.kind === "note" && row.id === renamingNoteId.value;
}

function cancelInlineRename() {
  renamingFolderPath.value = "";
  renamingFolderName.value = "";
  renamingNoteId.value = "";
  renamingNoteTitle.value = "";
}

function startInlineRename(path: string, currentName: string) {
  if (!path) {
    return;
  }

  renamingFolderPath.value = path;
  renamingFolderName.value = currentName;
  selectedRows.value = [`folder:${path}`];
  marqueeSelectionNoteId.value = "";
  closeContextMenu();

  void nextTick(() => {
    const input = treePanelRef.value?.querySelector<HTMLInputElement>(".folder-rename-input");
    input?.focus();
    input?.select();
  });
}

function startInlineRenameNote(noteId: string, currentTitle: string) {
  if (!noteId) {
    return;
  }

  renamingNoteId.value = noteId;
  renamingNoteTitle.value = currentTitle || "Untitled Note";
  renamingFolderPath.value = "";
  renamingFolderName.value = "";
  selectedRows.value = [`note:${noteId}`];
  marqueeSelectionNoteId.value = noteId;
  closeContextMenu();

  void nextTick(() => {
    const input = treePanelRef.value?.querySelector<HTMLInputElement>(".note-rename-input");
    input?.focus();
    input?.select();
  });
}

function commitInlineRename() {
  const path = renamingFolderPath.value;
  if (!path) {
    return;
  }

  const nextName = renamingFolderName.value.trim();
  const currentName = folderNameFromPath(path);
  if (!nextName || nextName === currentName) {
    cancelInlineRename();
    return;
  }

  const nextPath = folderPathWithName(path, nextName);
  selectedRows.value = [`folder:${nextPath}`];
  marqueeSelectionNoteId.value = "";
  emit("renameFolderInTree", path, nextName);
  cancelInlineRename();
}

function commitInlineRenameNote() {
  const noteId = renamingNoteId.value;
  if (!noteId) {
    return;
  }

  const nextTitle = renamingNoteTitle.value.trim();
  const currentTitle = props.notes.find((entry) => entry.id === noteId)?.title || "Untitled Note";
  if (!nextTitle || nextTitle === currentTitle) {
    cancelInlineRename();
    return;
  }

  emit("renameNoteInTree", noteId, nextTitle);
  cancelInlineRename();
}

function commitAnyInlineRename() {
  if (renamingFolderPath.value) {
    commitInlineRename();
    return;
  }

  if (renamingNoteId.value) {
    commitInlineRenameNote();
  }
}

function startInlineRenameFromSelection() {
  if (selectedRows.value.length !== 1) {
    return;
  }

  const key = selectedRows.value[0] || "";
  if (key.startsWith("folder:")) {
    const path = key.slice("folder:".length);
    if (!path) {
      return;
    }

    const row = visibleRows.value.find((entry) => entry.kind === "folder" && entry.path === path);
    const folderName = row?.kind === "folder" ? row.name : folderNameFromPath(path);
    startInlineRename(path, folderName);
    return;
  }

  if (!key.startsWith("note:")) {
    return;
  }

  const noteId = key.slice("note:".length);
  if (!noteId) {
    return;
  }

  const row = visibleRows.value.find((entry) => entry.kind === "note" && entry.id === noteId);
  const noteTitle = row?.kind === "note" ? row.title : props.notes.find((entry) => entry.id === noteId)?.title || "Untitled Note";
  startInlineRenameNote(noteId, noteTitle);
}

function setGlobalMarqueeTextSelection(disabled: boolean) {
  document.body.classList.toggle("fox-marquee-active", disabled);
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

function updateSelectionFromMarquee() {
  if (!marquee.active) {
    return;
  }

  const selectionTop = Math.min(marquee.startY, marquee.currentY);
  const selectionBottom = Math.max(marquee.startY, marquee.currentY);
  const normalizedTop = Math.min(selectionTop, selectionBottom - 1);
  const normalizedBottom = Math.max(selectionBottom, selectionTop + 1);

  const hits: string[] = [];
  let latestNoteId = "";

  const scope = treePanelRef.value ?? treeScrollRef.value;
  if (!scope) {
    selectedRows.value = [];
    marqueeSelectionNoteId.value = "";
    debugInfo.lastEvent = "update-no-scope";
    debugInfo.scope = "none";
    debugInfo.rowsFound = 0;
    debugInfo.rowsHit = 0;
    return;
  }

  const elements = Array.from(scope.querySelectorAll<HTMLElement>("[data-row-key]"));

  for (const element of elements) {
    const key = element.dataset.rowKey;
    if (!key) {
      continue;
    }

    const rect = element.getBoundingClientRect();
    const overlapsVertically = normalizedTop <= rect.bottom && normalizedBottom >= rect.top;
    if (overlapsVertically) {
      hits.push(key);
      const noteId = element.dataset.noteId;
      if (noteId) {
        latestNoteId = noteId;
      }
    }
  }

  selectedRows.value = hits;
  marqueeSelectionNoteId.value = latestNoteId;

  debugInfo.lastEvent = "update-selection";
  debugInfo.startX = marquee.startX;
  debugInfo.startY = marquee.startY;
  debugInfo.currentX = marquee.currentX;
  debugInfo.currentY = marquee.currentY;
  debugInfo.selectionTop = normalizedTop;
  debugInfo.selectionBottom = normalizedBottom;
  debugInfo.rowsFound = elements.length;
  debugInfo.rowsHit = hits.length;
  debugInfo.latestNoteId = latestNoteId;
  debugInfo.selectedCount = selectedRows.value.length;
  debugInfo.selectedPreview = selectedRows.value.slice(0, 6).join(", ");
  debugInfo.scope = scope === treePanelRef.value ? "panel" : "scroll";

  debugLog("updateSelectionFromMarquee", {
    selectionTop: normalizedTop,
    selectionBottom: normalizedBottom,
    rowsFound: elements.length,
    rowsHit: hits.length,
    latestNoteId,
    selectedRows: [...selectedRows.value],
  });
}

function onTreeMouseDown(event: MouseEvent) {
  if (marquee.pending || marquee.active) {
    return;
  }

  if (event.button !== 0) {
    return;
  }

  const targetEl = resolveTargetElement(event.target);
  if (!targetEl) {
    return;
  }

  if (targetEl.closest("input, textarea, [contenteditable='true'], .v-field, .context-menu")) {
    debugInfo.lastEvent = "mousedown-ignored";
    debugInfo.targetTag = targetEl.tagName;
    debugInfo.targetClasses = targetEl.className;
    debugLog("mousedown ignored by filter", {
      tag: targetEl.tagName,
      className: targetEl.className,
    });
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

  debugInfo.lastEvent = "mousedown-start";
  debugInfo.startX = event.clientX;
  debugInfo.startY = event.clientY;
  debugInfo.currentX = event.clientX;
  debugInfo.currentY = event.clientY;
  debugInfo.targetTag = targetEl.tagName;
  debugInfo.targetClasses = targetEl.className;
  debugLog("mousedown start", {
    startX: event.clientX,
    startY: event.clientY,
    tag: targetEl.tagName,
    className: targetEl.className,
  });
}

function onWindowMouseMove(event: MouseEvent) {
  if (marquee.pending && !marquee.active) {
    const movedDistance = Math.hypot(event.clientX - marquee.startX, event.clientY - marquee.startY);
    if (movedDistance > 4) {
      marquee.pending = false;
      marquee.active = true;
      clearTreeSelection();
      debugInfo.lastEvent = "move-activate-marquee";
      debugLog("marquee activated", {
        movedDistance,
        startX: marquee.startX,
        startY: marquee.startY,
      });
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

function onWindowMouseUp(event: MouseEvent) {
  if (marquee.pending && !marquee.active) {
    const movedDistanceWhilePending = Math.hypot(event.clientX - marquee.startX, event.clientY - marquee.startY);
    if (movedDistanceWhilePending > 4) {
      marquee.pending = false;
      marquee.active = true;
      marquee.currentX = event.clientX;
      marquee.currentY = event.clientY;
      clearTreeSelection();
      updateSelectionFromMarquee();
    } else {
      marquee.pending = false;
      setGlobalMarqueeTextSelection(false);
      marquee.justFinished = false;
      debugInfo.lastEvent = "mouseup-cancel-pending";
      debugLog("mouseup cancelled pending marquee", {
        movedDistanceWhilePending,
      });
      return;
    }
  }

  if (!marquee.active) {
    return;
  }

  marquee.currentX = event.clientX;
  marquee.currentY = event.clientY;
  updateSelectionFromMarquee();

  const movedDistance = Math.hypot(marquee.currentX - marquee.startX, marquee.currentY - marquee.startY);
  marquee.active = false;
  marquee.pending = false;
  setGlobalMarqueeTextSelection(false);
  marquee.justFinished = true;

  debugInfo.lastEvent = "mouseup-finish";
  debugInfo.currentX = marquee.currentX;
  debugInfo.currentY = marquee.currentY;
  debugInfo.selectedCount = selectedRows.value.length;
  debugInfo.latestNoteId = marqueeSelectionNoteId.value;
  debugLog("mouseup finish", {
    movedDistance,
    selectedCount: selectedRows.value.length,
    latestNoteId: marqueeSelectionNoteId.value,
    selectedRows: [...selectedRows.value],
  });

  if (movedDistance <= 4 || !marqueeSelectionNoteId.value) {
    return;
  }

  emit("selectNote", marqueeSelectionNoteId.value);
}

function onWindowKeydown(event: KeyboardEvent) {
  if (event.key !== "F2") {
    return;
  }

  if (isTextEntryTarget(event.target)) {
    return;
  }

  if (renamingFolderPath.value) {
    return;
  }

  if (renamingNoteId.value) {
    return;
  }

  event.preventDefault();
  startInlineRenameFromSelection();
}

function onTreeClickCapture(event: MouseEvent) {
  const targetEl = resolveTargetElement(event.target);
  if (
    (renamingFolderPath.value || renamingNoteId.value) &&
    targetEl &&
    !targetEl.closest(".folder-rename-input") &&
    !targetEl.closest(".note-rename-input")
  ) {
    commitAnyInlineRename();
  }

  if (!marquee.justFinished) {
    if (!targetEl) {
      clearTreeSelection();
      return;
    }

    if (targetEl.closest(".context-menu")) {
      return;
    }

    if (!targetEl.closest(".tree-row")) {
      clearTreeSelection();
    }
    return;
  }

  event.preventDefault();
  event.stopPropagation();
  marquee.justFinished = false;
}

function onRowClick(row: TreeRow) {
  selectSingleRow(row);
  if (row.kind === "note") {
    emit("selectNote", row.id);
  }
}

function openContextMenu(event: MouseEvent, kind: "folder" | "note", payload: Record<string, string>) {
  const maxX = Math.max(window.innerWidth - 226, 8);
  const maxY = Math.max(window.innerHeight - 116, 8);

  contextMenu.open = true;
  contextMenu.kind = kind;
  contextMenu.folderPath = payload.folderPath ?? "";
  contextMenu.folderName = payload.folderName ?? "root";
  contextMenu.noteId = payload.noteId ?? "";
  contextMenu.noteTitle = payload.noteTitle ?? "";
  contextMenu.x = Math.min(event.clientX, maxX);
  contextMenu.y = Math.min(event.clientY, maxY);
}

function onRootContextMenu(event: MouseEvent) {
  event.preventDefault();
  clearTreeSelection();
  openContextMenu(event, "folder", {
    folderPath: "",
    folderName: "root",
  });
}

function onRowContextMenu(event: MouseEvent, row: TreeRow) {
  event.preventDefault();

  const key = rowKey(row);
  if (!selectedRowSet.value.has(key) || selectedRows.value.length <= 1) {
    selectSingleRow(row);
  }

  if (row.kind === "folder") {
    openContextMenu(event, "folder", {
      folderPath: row.path,
      folderName: row.name,
    });
    return;
  }

  openContextMenu(event, "note", {
    folderPath: row.folder,
    folderName: row.folder || "root",
    noteId: row.id,
    noteTitle: row.title,
  });
}

function emitDeleteSelectionFromContext() {
  const payload = {
    noteIds: selectedTargets.value.notes.map((entry) => entry.id),
    noteTitles: selectedTargets.value.notes.map((entry) => entry.title),
    folderPaths: selectedTargets.value.folders.map((entry) => entry.path),
    folderNames: selectedTargets.value.folders.map((entry) => entry.name),
  };

  if (payload.noteIds.length + payload.folderPaths.length === 0) {
    return;
  }

  emit("deleteTreeSelection", payload);
  closeContextMenu();
}

function onDeleteActionFromContext() {
  if (usingMultiSelectionContext.value) {
    emitDeleteSelectionFromContext();
    return;
  }

  if (contextMenu.kind === "folder") {
    deleteFolderFromContext();
    return;
  }

  deleteNoteFromContext();
}

function isExpanded(path: string): boolean {
  return expanded.value[path] ?? true;
}

function toggleFolder(path: string) {
  expanded.value = {
    ...expanded.value,
    [path]: !isExpanded(path),
  };
}

function closeContextMenu() {
  contextMenu.open = false;
}

function createFolderFromContext() {
  emit("createFolderInFolder", contextMenu.folderPath);
  closeContextMenu();
}

function renameFolderFromContext() {
  if (!contextMenu.folderPath || usingMultiSelectionContext.value) {
    return;
  }

  startInlineRename(contextMenu.folderPath, contextMenu.folderName);
}

function renameNoteFromContext() {
  if (!contextMenu.noteId || usingMultiSelectionContext.value) {
    return;
  }

  startInlineRenameNote(contextMenu.noteId, contextMenu.noteTitle || "Untitled Note");
}

function deleteFolderFromContext() {
  if (!contextMenu.folderPath) {
    return;
  }
  emit("deleteFolderInTree", contextMenu.folderPath, contextMenu.folderName);
  closeContextMenu();
}

function createNoteFromContext() {
  emit("createNoteInFolder", contextMenu.folderPath);
  closeContextMenu();
}

function createNoteInFolderRow(folderPath: string) {
  emit("createNoteInFolder", folderPath);
}

function deleteNoteFromContext() {
  if (!contextMenu.noteId) {
    return;
  }
  emit("deleteNoteInTree", contextMenu.noteId, contextMenu.noteTitle || "Untitled Note");
  closeContextMenu();
}
</script>

<template>
  <section
    ref="treePanelRef"
    class="tree-panel"
    :class="{ 'marquee-active': marquee.pending || marquee.active }"
    @mousedown.capture="onTreeMouseDown"
    @click.capture="onTreeClickCapture"
  >
    <div class="tree-header" @click="clearTreeSelection" @contextmenu="onRootContextMenu($event)">
      <div class="root-row">
        <v-icon icon="mdi-home-outline" size="16" />
        <span>root</span>
        <button
          type="button"
          class="folder-add-btn"
          title="Create note in root"
          @click.stop="createNoteInFolderRow('')"
        >
          <v-icon icon="mdi-plus" size="14" />
        </button>
      </div>
      <v-progress-circular v-if="loading" indeterminate size="15" width="2" color="primary" />
    </div>

    <div ref="treeScrollRef" class="tree-scroll">
      <div
        v-for="row in visibleRows"
        :key="row.kind === 'folder' ? `folder-${row.path}` : `note-${row.id}`"
        class="tree-row"
        :data-row-key="rowKey(row)"
        :data-note-id="row.kind === 'note' ? row.id : ''"
        :class="{
          'folder-row': row.kind === 'folder',
          'note-row': row.kind === 'note',
          'row-selected': isRowSelected(row),
          'note-active': row.kind === 'note' && row.id === selectedNoteId,
        }"
        :style="rowStyle(row.depth)"
        @click="onRowClick(row)"
        @contextmenu="onRowContextMenu($event, row)"
      >
        <button
          v-if="row.kind === 'folder'"
          type="button"
          class="disclosure"
          :disabled="!row.hasChildren"
          @click.stop="toggleFolder(row.path)"
        >
          <v-icon
            v-if="row.hasChildren"
            :icon="isExpanded(row.path) ? 'mdi-chevron-down' : 'mdi-chevron-right'"
            size="16"
          />
        </button>
        <span v-else class="disclosure-placeholder" />

        <v-icon v-if="row.kind === 'folder'" icon="mdi-folder-outline" size="16" />
        <v-icon v-else icon="mdi-note-text-outline" size="15" />

        <input
          v-if="row.kind === 'folder' && isRenamingFolderRow(row)"
          v-model="renamingFolderName"
          class="row-label folder-rename-input"
          type="text"
          @mousedown.stop
          @click.stop
          @keydown.enter.prevent.stop="commitInlineRename"
          @keydown.esc.prevent.stop="cancelInlineRename"
        />
        <input
          v-else-if="row.kind === 'note' && isRenamingNoteRow(row)"
          v-model="renamingNoteTitle"
          class="row-label note-rename-input"
          type="text"
          @mousedown.stop
          @click.stop
          @keydown.enter.prevent.stop="commitInlineRenameNote"
          @keydown.esc.prevent.stop="cancelInlineRename"
        />
        <span v-else class="row-label">{{ row.kind === "folder" ? row.name : row.title }}</span>

        <button
          v-if="row.kind === 'folder'"
          type="button"
          class="folder-add-btn"
          title="Create note in folder"
          @click.stop="createNoteInFolderRow(row.path)"
        >
          <v-icon icon="mdi-plus" size="14" />
        </button>
      </div>

    </div>

    <div class="tree-marquee" :style="marqueeStyle" />

    <pre v-if="DEBUG_TREE_MARQUEE" class="tree-debug">
event={{ debugInfo.lastEvent }}
start=({{ Math.round(debugInfo.startX) }}, {{ Math.round(debugInfo.startY) }}) current=({{ Math.round(debugInfo.currentX) }}, {{ Math.round(debugInfo.currentY) }})
selectionY={{ Math.round(debugInfo.selectionTop) }}..{{ Math.round(debugInfo.selectionBottom) }} scope={{ debugInfo.scope }}
rowsFound={{ debugInfo.rowsFound }} rowsHit={{ debugInfo.rowsHit }} selected={{ debugInfo.selectedCount }} latestNote={{ debugInfo.latestNoteId || "-" }}
target={{ debugInfo.targetTag || "-" }} classes={{ debugInfo.targetClasses || "-" }}
selectedPreview={{ debugInfo.selectedPreview || "-" }}
    </pre>

    <p v-if="notes.length === 0" class="empty-state">No notes yet. Right-click root to create content.</p>

    <div v-if="contextMenu.open" class="context-overlay" @click="closeContextMenu">
      <div class="context-menu" :style="contextMenuStyle" @click.stop>
        <template v-if="contextMenu.kind === 'folder'">
          <button v-if="!usingMultiSelectionContext" type="button" class="context-action" @click="createNoteFromContext">
            <v-icon icon="mdi-note-plus-outline" size="16" />
            <span>Create note in {{ contextMenu.folderName }}</span>
          </button>
          <button v-if="!usingMultiSelectionContext" type="button" class="context-action" @click="createFolderFromContext">
            <v-icon icon="mdi-folder-plus-outline" size="16" />
            <span>Create folder in {{ contextMenu.folderName }}</span>
          </button>
          <button
            v-if="contextMenu.folderPath && !usingMultiSelectionContext"
            type="button"
            class="context-action"
            @click="renameFolderFromContext"
          >
            <v-icon icon="mdi-pencil-outline" size="16" />
            <span>Rename {{ contextMenu.folderName }}</span>
          </button>
          <button
            v-if="contextMenu.folderPath"
            type="button"
            class="context-action danger"
            @click="onDeleteActionFromContext"
          >
            <v-icon icon="mdi-folder-remove-outline" size="16" />
            <span>
              {{
                usingMultiSelectionContext
                  ? `Delete selection (${selectedItemCount})`
                  : `Delete ${contextMenu.folderName}`
              }}
            </span>
          </button>
        </template>
        <template v-else>
          <button
            v-if="!usingMultiSelectionContext"
            type="button"
            class="context-action"
            @click="renameNoteFromContext"
          >
            <v-icon icon="mdi-pencil-outline" size="16" />
            <span>Rename {{ contextMenu.noteTitle }}</span>
          </button>
          <button type="button" class="context-action danger" @click="onDeleteActionFromContext">
            <v-icon icon="mdi-delete-outline" size="16" />
            <span>
              {{
                usingMultiSelectionContext
                  ? `Delete selection (${selectedItemCount})`
                  : `Delete ${contextMenu.noteTitle}`
              }}
            </span>
          </button>
        </template>
      </div>
    </div>
  </section>
</template>

<style scoped>
.tree-panel {
  border: 0;
  border-radius: 0;
  background: transparent;
  min-height: 100%;
  padding: 0.2rem 0.15rem 0.35rem;
  user-select: none;
  position: relative;
}

.tree-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.25rem 0.55rem 0.35rem;
}

.root-row {
  display: flex;
  align-items: center;
  gap: 0.42rem;
  font-size: 1.02rem;
  color: var(--fox-text-strong);
}

.tree-scroll {
  display: flex;
  flex-direction: column;
  gap: 0.08rem;
  position: relative;
}

.tree-row {
  width: 100%;
  box-sizing: border-box;
  min-height: 2rem;
  display: flex;
  align-items: center;
  gap: 0.38rem;
  border-radius: 8px;
  color: var(--fox-text-body);
  cursor: default;
}

.folder-row {
  color: color-mix(in srgb, var(--fox-text-body) 80%, var(--fox-text-muted) 20%);
}

.note-row {
  cursor: pointer;
}

.tree-row:hover {
  background: color-mix(in srgb, var(--fox-chip) 84%, transparent 16%);
}

.note-active {
  background: color-mix(in srgb, var(--fox-chip) 64%, #33405c 36%);
}

.tree-row.row-selected {
  background: color-mix(in srgb, #2f8bff 36%, var(--fox-chip) 64%);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, #76bcff 55%, transparent 45%);
}

.tree-row.row-selected:hover {
  background: color-mix(in srgb, #2f8bff 42%, var(--fox-chip) 58%);
}

.tree-row.row-selected.note-active {
  background: color-mix(in srgb, #2f8bff 48%, #33405c 52%);
}

.disclosure,
.disclosure-placeholder {
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 18px;
}

.disclosure {
  border: 0;
  background: transparent;
  color: var(--fox-text-muted);
  cursor: pointer;
  padding: 0;
}

.disclosure:disabled {
  cursor: default;
}

.row-label {
  min-width: 0;
  flex: 1 1 auto;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.folder-rename-input,
.note-rename-input {
  height: 1.5rem;
  border: 1px solid color-mix(in srgb, #76bcff 60%, transparent 40%);
  border-radius: 5px;
  background: color-mix(in srgb, var(--fox-surface) 82%, #112038 18%);
  color: var(--fox-text-strong);
  padding: 0 0.42rem;
  outline: none;
}

.folder-rename-input:focus,
.note-rename-input:focus {
  border-color: #76bcff;
  box-shadow: 0 0 0 1px color-mix(in srgb, #76bcff 45%, transparent 55%);
}

.folder-add-btn {
  margin-left: auto;
  width: 20px;
  height: 20px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--fox-text-muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex: 0 0 auto;
}

.folder-add-btn:hover {
  background: color-mix(in srgb, var(--fox-chip) 85%, transparent 15%);
  color: var(--fox-text-strong);
}

.empty-state {
  margin: 0.6rem 0.5rem 0.1rem;
  font-size: 0.84rem;
  color: var(--fox-text-muted);
}

.context-overlay {
  position: fixed;
  inset: 0;
  z-index: 40;
}

.context-menu {
  position: fixed;
  width: 226px;
  border: 1px solid var(--fox-border);
  border-radius: 10px;
  background: color-mix(in srgb, var(--fox-surface) 88%, black 12%);
  box-shadow: 0 10px 22px rgba(0, 0, 0, 0.35);
  padding: 0.3rem;
}

.context-action {
  width: 100%;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--fox-text-body);
  padding: 0.42rem 0.5rem;
  display: flex;
  align-items: center;
  gap: 0.45rem;
  text-align: left;
  cursor: pointer;
}

.context-action:hover {
  background: var(--fox-chip);
}

.context-action.danger:hover {
  background: rgba(212, 73, 73, 0.2);
  color: #ffb9b9;
}

.tree-marquee {
  position: absolute;
  border: 1px solid #3aa5ff;
  background: color-mix(in srgb, #2f8bff 26%, transparent 74%);
  border-radius: 6px;
  pointer-events: none;
  z-index: 12;
}

.tree-debug {
  margin: 0.5rem 0.35rem 0;
  border: 1px solid color-mix(in srgb, #3aa5ff 40%, transparent 60%);
  border-radius: 6px;
  background: color-mix(in srgb, #0f1826 80%, black 20%);
  color: #9fceff;
  padding: 0.4rem 0.48rem;
  font-size: 0.72rem;
  line-height: 1.3;
  white-space: pre-wrap;
}
</style>
