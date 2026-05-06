<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
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
  deleteFolderInTree: [folderPath: string, folderName: string];
  deleteNoteInTree: [noteId: string, noteTitle: string];
}>();

const expanded = ref<Record<string, boolean>>({});

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

function rowStyle(depth: number): Record<string, string> {
  return {
    paddingLeft: `${depth * 1.05 + 0.25}rem`,
  };
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

function onFolderContextMenu(event: MouseEvent, folderPath: string, folderName: string) {
  event.preventDefault();
  const maxX = Math.max(window.innerWidth - 226, 8);
  const maxY = Math.max(window.innerHeight - 116, 8);

  contextMenu.open = true;
  contextMenu.kind = "folder";
  contextMenu.folderPath = folderPath;
  contextMenu.folderName = folderName;
  contextMenu.noteId = "";
  contextMenu.noteTitle = "";
  contextMenu.x = Math.min(event.clientX, maxX);
  contextMenu.y = Math.min(event.clientY, maxY);
}

function onNoteContextMenu(event: MouseEvent, noteId: string, noteTitle: string, folder: string) {
  event.preventDefault();
  const maxX = Math.max(window.innerWidth - 226, 8);
  const maxY = Math.max(window.innerHeight - 116, 8);

  contextMenu.open = true;
  contextMenu.kind = "note";
  contextMenu.folderPath = folder;
  contextMenu.folderName = folder || "root";
  contextMenu.noteId = noteId;
  contextMenu.noteTitle = noteTitle;
  contextMenu.x = Math.min(event.clientX, maxX);
  contextMenu.y = Math.min(event.clientY, maxY);
}

function closeContextMenu() {
  contextMenu.open = false;
}

function createFolderFromContext() {
  emit("createFolderInFolder", contextMenu.folderPath);
  closeContextMenu();
}

function renameFolderFromContext() {
  if (!contextMenu.folderPath) {
    return;
  }
  emit("renameFolderInTree", contextMenu.folderPath, contextMenu.folderName);
  closeContextMenu();
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

function deleteNoteFromContext() {
  if (!contextMenu.noteId) {
    return;
  }
  emit("deleteNoteInTree", contextMenu.noteId, contextMenu.noteTitle || "Untitled Note");
  closeContextMenu();
}
</script>

<template>
  <section class="tree-panel">
    <div class="tree-header" @contextmenu="onFolderContextMenu($event, '', 'root')">
      <div class="root-row">
        <v-icon icon="mdi-home-outline" size="16" />
        <span>root</span>
      </div>
      <v-progress-circular v-if="loading" indeterminate size="15" width="2" color="primary" />
    </div>

    <div class="tree-scroll">
      <div
        v-for="row in visibleRows"
        :key="row.kind === 'folder' ? `folder-${row.path}` : `note-${row.id}`"
        class="tree-row"
        :class="{
          'folder-row': row.kind === 'folder',
          'note-row': row.kind === 'note',
          'note-active': row.kind === 'note' && row.id === selectedNoteId,
        }"
        :style="rowStyle(row.depth)"
        @click="row.kind === 'note' && emit('selectNote', row.id)"
        @contextmenu="
          row.kind === 'folder'
            ? onFolderContextMenu($event, row.path, row.name)
            : onNoteContextMenu($event, row.id, row.title, row.folder)
        "
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

        <span class="row-label">{{ row.kind === "folder" ? row.name : row.title }}</span>
      </div>
    </div>

    <p v-if="notes.length === 0" class="empty-state">No notes yet. Right-click root to create content.</p>

    <div v-if="contextMenu.open" class="context-overlay" @click="closeContextMenu">
      <div class="context-menu" :style="contextMenuStyle" @click.stop>
        <template v-if="contextMenu.kind === 'folder'">
          <button type="button" class="context-action" @click="createNoteFromContext">
            <v-icon icon="mdi-note-plus-outline" size="16" />
            <span>Create note in {{ contextMenu.folderName }}</span>
          </button>
          <button type="button" class="context-action" @click="createFolderFromContext">
            <v-icon icon="mdi-folder-plus-outline" size="16" />
            <span>Create folder in {{ contextMenu.folderName }}</span>
          </button>
          <button
            v-if="contextMenu.folderPath"
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
            @click="deleteFolderFromContext"
          >
            <v-icon icon="mdi-folder-remove-outline" size="16" />
            <span>Delete {{ contextMenu.folderName }}</span>
          </button>
        </template>
        <button v-else type="button" class="context-action danger" @click="deleteNoteFromContext">
          <v-icon icon="mdi-delete-outline" size="16" />
          <span>Delete {{ contextMenu.noteTitle }}</span>
        </button>
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
}

.tree-row {
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
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
</style>
