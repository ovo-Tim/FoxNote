<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import NoteEditorPanel from "./components/NoteEditorPanel.vue";
import NoteTreePanel from "./components/NoteTreePanel.vue";
import { useTagFilter } from "./composables/useTagFilter";
import {
  createNote,
  createNoteFolder,
  deleteNote,
  deleteNoteFolder,
  getNoteStorageRoot,
  getTagBridgePath,
  listNoteIdsForTag,
  listNoteTree,
  listTags,
  loadNote,
  renameNoteFolder,
  saveNote,
} from "./lib/noteApi";
import type { NoteDocument, NoteRecord, NoteSummary, NoteTree } from "./types/note";

const tree = ref<NoteTree>({ folders: [], notes: [] });
const selectedNote = ref<NoteRecord | null>(null);
const selectedNoteId = ref<string>("");
const noteStorageRoot = ref<string>("");
const tagBridgePath = ref<string>("");
const loadingTree = ref(false);
const busy = ref(false);
const notice = ref<string>("");
const error = ref<string>("");
const searchQuery = ref<string>("");
const deleteDialogOpen = ref(false);
const pendingDeleteId = ref("");
const pendingDeleteTitle = ref("");
const folderRenameDialogOpen = ref(false);
const pendingRenameFolderPath = ref("");
const pendingRenameFolderName = ref("");
const folderDeleteDialogOpen = ref(false);
const pendingDeleteFolderPath = ref("");
const pendingDeleteFolderName = ref("");

const {
  activeTag,
  clearFilter,
  filteredNotes,
  isFiltering,
  setActiveTag,
  setTags,
  tags,
} = useTagFilter();

const notesForTree = computed<NoteSummary[]>(() => {
  const base = filteredNotes.value(tree.value.notes);
  if (!searchQuery.value.trim()) {
    return base;
  }

  const query = searchQuery.value.toLowerCase();
  return base.filter((note) => {
    const text = [
      note.title,
      note.folder,
      note.date,
      note.tags.join(" "),
      note.type,
    ]
      .join(" ")
      .toLowerCase();
    return text.includes(query);
  });
});

const visibleFolderPaths = computed<Set<string>>(() => {
  const paths = new Set<string>();

  for (const note of notesForTree.value) {
    const segments = note.folder.split("/").filter(Boolean);
    let cursor = "";

    for (const segment of segments) {
      cursor = cursor ? `${cursor}/${segment}` : segment;
      paths.add(cursor);
    }
  }

  if (!isFiltering.value && !searchQuery.value.trim()) {
    return new Set(tree.value.folders.map((folder) => folder.path));
  }

  return paths;
});

const foldersForTree = computed(() => {
  return tree.value.folders.filter((folder) => visibleFolderPaths.value.has(folder.path));
});

const showEmptyFolders = computed(() => {
  return !isFiltering.value && !searchQuery.value.trim();
});

const tagOptions = computed(() => {
  return tags.value.map((tag) => tag.path);
});

async function bootstrap() {
  loadingTree.value = true;
  error.value = "";

  try {
    noteStorageRoot.value = await getNoteStorageRoot();
    tagBridgePath.value = await getTagBridgePath();
    await refreshTree();
    await refreshTags();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    loadingTree.value = false;
  }
}

async function refreshTree() {
  loadingTree.value = true;

  try {
    tree.value = await listNoteTree();

    if (selectedNoteId.value) {
      const stillExists = tree.value.notes.some((note) => note.id === selectedNoteId.value);
      if (!stillExists) {
        selectedNoteId.value = "";
        selectedNote.value = null;
      }
    }
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    loadingTree.value = false;
  }
}

async function refreshTags() {
  try {
    const nextTags = await listTags();
    setTags(nextTags);

    if (activeTag.value) {
      const current = nextTags.find((tag) => tag.path === activeTag.value);
      if (!current) {
        clearFilter();
        return;
      }
      const noteIds = await listNoteIdsForTag(activeTag.value);
      setActiveTag(activeTag.value, noteIds);
    }
  } catch (reason) {
    error.value = toErrorMessage(reason);
  }
}

async function selectNote(id: string) {
  selectedNoteId.value = id;
  error.value = "";

  try {
    selectedNote.value = await loadNote(id);
  } catch (reason) {
    error.value = toErrorMessage(reason);
  }
}

async function createDefaultNote(folderPath = "") {
  const folder = folderPath.trim();
  busy.value = true;
  error.value = "";

  try {
    const created = await createNote(folder, "Untitled Note");
    await refreshTree();
    await refreshTags();
    const locationLabel = folder ? ` in '${folder}'` : "";

    if (searchQuery.value.trim() || isFiltering.value) {
      searchQuery.value = "";
      clearFilter();
      notice.value = `Created note '${created.document.title}'${locationLabel}. Cleared filters to show it.`;
    } else {
      notice.value = `Created note '${created.document.title}'${locationLabel}.`;
    }

    await selectNote(created.id);
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    busy.value = false;
  }
}

function handleCreateNoteInFolder(folderPath: string) {
  void createDefaultNote(folderPath);
}

function nextFolderName(parentPath: string): string {
  const existing = new Set(
    tree.value.folders
      .filter((folder) => {
        const folderParent = folder.path.includes("/") ? folder.path.slice(0, folder.path.lastIndexOf("/")) : "";
        return folderParent === parentPath;
      })
      .map((folder) => folder.name),
  );

  const base = "new-folder";
  if (!existing.has(base)) {
    return base;
  }

  let index = 2;
  while (existing.has(`${base}-${index}`)) {
    index += 1;
  }

  return `${base}-${index}`;
}

async function createFolderInPath(parentPath: string) {
  const parent = parentPath.trim();
  const name = nextFolderName(parent);
  busy.value = true;
  error.value = "";

  try {
    const created = await createNoteFolder(parent, name);
    notice.value = `Created folder '${created.path}'.`;
    await refreshTree();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    busy.value = false;
  }
}

function handleCreateFolderInFolder(parentPath: string) {
  void createFolderInPath(parentPath);
}

async function handleSave(document: NoteDocument) {
  if (!selectedNoteId.value) {
    return;
  }

  if (!selectedNote.value) {
    return;
  }

  const current = JSON.stringify(selectedNote.value.document);
  const next = JSON.stringify(document);
  if (current === next) {
    return;
  }

  error.value = "";

  try {
    await saveNote(selectedNoteId.value, document);
    await refreshTree();
    await refreshTags();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  }
}

async function handleDeleteFromTree(noteId: string, noteTitle: string) {
  pendingDeleteId.value = noteId;
  pendingDeleteTitle.value = noteTitle;
  deleteDialogOpen.value = true;
}

function handleRenameFolderInTree(folderPath: string, folderName: string) {
  pendingRenameFolderPath.value = folderPath;
  pendingRenameFolderName.value = folderName;
  folderRenameDialogOpen.value = true;
}

function cancelRenameFolder() {
  folderRenameDialogOpen.value = false;
  pendingRenameFolderPath.value = "";
  pendingRenameFolderName.value = "";
}

async function confirmRenameFolder() {
  const path = pendingRenameFolderPath.value;
  const nextName = pendingRenameFolderName.value.trim();
  if (!path) {
    cancelRenameFolder();
    return;
  }

  if (!nextName) {
    error.value = "Folder name cannot be empty.";
    return;
  }

  folderRenameDialogOpen.value = false;
  busy.value = true;
  error.value = "";

  try {
    const renamed = await renameNoteFolder(path, nextName);

    if (selectedNoteId.value && selectedNoteId.value.startsWith(`${path}/`)) {
      const nextPrefix = `${renamed.path}/`;
      selectedNoteId.value = `${nextPrefix}${selectedNoteId.value.slice(path.length + 1)}`;
      await selectNote(selectedNoteId.value);
    }

    if (pendingDeleteFolderPath.value === path) {
      pendingDeleteFolderPath.value = renamed.path;
    }

    notice.value = `Renamed folder to '${renamed.path}'.`;
    await refreshTree();
    await refreshTags();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    busy.value = false;
    pendingRenameFolderPath.value = "";
    pendingRenameFolderName.value = "";
  }
}

function handleDeleteFolderInTree(folderPath: string, folderName: string) {
  pendingDeleteFolderPath.value = folderPath;
  pendingDeleteFolderName.value = folderName;
  folderDeleteDialogOpen.value = true;
}

function cancelDeleteFolder() {
  folderDeleteDialogOpen.value = false;
  pendingDeleteFolderPath.value = "";
  pendingDeleteFolderName.value = "";
}

async function confirmDeleteFolder() {
  const folderPath = pendingDeleteFolderPath.value;
  const folderName = pendingDeleteFolderName.value || "folder";
  if (!folderPath) {
    cancelDeleteFolder();
    return;
  }

  folderDeleteDialogOpen.value = false;
  busy.value = true;
  error.value = "";

  try {
    await deleteNoteFolder(folderPath);
    if (selectedNoteId.value && selectedNoteId.value.startsWith(`${folderPath}/`)) {
      selectedNoteId.value = "";
      selectedNote.value = null;
    }
    notice.value = `Deleted folder '${folderName}'.`;
    await refreshTree();
    await refreshTags();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    busy.value = false;
    pendingDeleteFolderPath.value = "";
    pendingDeleteFolderName.value = "";
  }
}

function cancelDeleteNote() {
  deleteDialogOpen.value = false;
  pendingDeleteId.value = "";
  pendingDeleteTitle.value = "";
}

async function confirmDeleteNote() {
  if (!pendingDeleteId.value) {
    cancelDeleteNote();
    return;
  }

  const noteId = pendingDeleteId.value;
  const noteTitle = pendingDeleteTitle.value || "Untitled Note";
  deleteDialogOpen.value = false;

  busy.value = true;
  error.value = "";

  try {
    await deleteNote(noteId);
    notice.value = `Deleted '${noteTitle}'.`;

    if (selectedNoteId.value === noteId) {
      selectedNote.value = null;
      selectedNoteId.value = "";
    }

    await refreshTree();
    await refreshTags();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    busy.value = false;
    pendingDeleteId.value = "";
    pendingDeleteTitle.value = "";
  }
}

function toErrorMessage(reason: unknown): string {
  if (typeof reason === "string") {
    return reason;
  }
  if (reason instanceof Error) {
    return reason.message;
  }
  return "Unexpected error";
}

onMounted(() => {
  void bootstrap();
});
</script>

<template>
  <v-app>
    <v-main class="app-shell">
      <div class="page-wrap">
        <v-alert v-if="notice" type="success" variant="tonal" class="notice-row" closable @click:close="notice = ''">
          {{ notice }}
        </v-alert>

        <v-alert v-if="error" type="error" variant="tonal" class="notice-row" closable @click:close="error = ''">
          {{ error }}
        </v-alert>

        <section class="workspace-grid">
          <aside class="sidebar-column">
            <NoteTreePanel :folders="foldersForTree" :notes="notesForTree" :selected-note-id="selectedNoteId"
              :loading="loadingTree || busy" :show-empty-folders="showEmptyFolders" @select-note="selectNote"
              @create-note-in-folder="handleCreateNoteInFolder" @create-folder-in-folder="handleCreateFolderInFolder"
              @rename-folder-in-tree="handleRenameFolderInTree" @delete-folder-in-tree="handleDeleteFolderInTree"
              @delete-note-in-tree="handleDeleteFromTree" />
          </aside>

          <article class="editor-column">
            <NoteEditorPanel :note="selectedNote" :tag-options="tagOptions" @change="handleSave" />
          </article>
        </section>

        <footer class="footer">
          <span>Storage root: <code>{{ noteStorageRoot || 'loading...' }}</code></span>
          <span>Tag bridge: <code>{{ tagBridgePath || 'loading...' }}</code></span>
          <span>Wave 2: nested tags + SQLite bridge + tag filtering.</span>
        </footer>
      </div>
    </v-main>

    <v-dialog v-model="deleteDialogOpen" max-width="460">
      <v-card>
        <v-card-title>Delete note?</v-card-title>
        <v-card-text>
          Delete '{{ pendingDeleteTitle || "Untitled Note" }}' and its assets? This cannot be undone.
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="cancelDeleteNote">Cancel</v-btn>
          <v-btn color="error" variant="flat" :loading="busy" @click="confirmDeleteNote">Delete</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="folderRenameDialogOpen" max-width="460">
      <v-card>
        <v-card-title>Rename folder</v-card-title>
        <v-card-text>
          <v-text-field v-model="pendingRenameFolderName" label="Folder name" variant="outlined" density="comfortable"
            autofocus @keydown.enter="confirmRenameFolder" />
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="cancelRenameFolder">Cancel</v-btn>
          <v-btn color="primary" variant="flat" :loading="busy" @click="confirmRenameFolder">Rename</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="folderDeleteDialogOpen" max-width="500">
      <v-card>
        <v-card-title>Delete folder?</v-card-title>
        <v-card-text>
          Delete folder '{{ pendingDeleteFolderName || "folder" }}' and all notes inside? This cannot be undone.
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="cancelDeleteFolder">Cancel</v-btn>
          <v-btn color="error" variant="flat" :loading="busy" @click="confirmDeleteFolder">Delete</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

  </v-app>
</template>

<style scoped>
.app-shell {
  padding: 0;
}

.page-wrap {
  width: 100%;
  margin: 0;
  animation: rise-in 0.38s ease-out;
}

.notice-row {
  margin-top: 0.55rem;
}

.workspace-grid {
  margin-top: 0;
  display: grid;
  gap: 0;
  grid-template-columns: 290px minmax(0, 1fr);
  min-height: 100vh;
}

.sidebar-column {
  min-width: 0;
  border-right: 1px solid var(--fox-border);
  padding: 0.5rem 0.25rem 0.45rem 0.4rem;
}

.editor-column {
  min-width: 0;
  padding: 0.55rem 0.75rem 0.55rem 0.85rem;
}

.footer {
  margin-top: 0;
  display: flex;
  justify-content: space-between;
  gap: 0.8rem;
  flex-wrap: wrap;
  color: var(--fox-text-muted);
  font-size: 0.8rem;
  border-top: 1px solid var(--fox-border);
  padding: 0.35rem 0.8rem;
}

@media (max-width: 980px) {
  .workspace-grid {
    grid-template-columns: 1fr;
    min-height: auto;
  }
}

@keyframes rise-in {
  from {
    transform: translateY(8px);
    opacity: 0;
  }

  to {
    transform: translateY(0);
    opacity: 1;
  }
}
</style>
