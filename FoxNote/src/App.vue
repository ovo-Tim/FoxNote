<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import NoteEditorPanel from "./components/NoteEditorPanel.vue";
import NoteTreePanel from "./components/NoteTreePanel.vue";
import PluginsPanel from "./components/PluginsPanel.vue";
import SearchPage from "./components/SearchPage.vue";
import SyncPanel from "./components/SyncPanel.vue";
import { useTagFilter } from "./composables/useTagFilter";
import {
  createNote,
  createNoteFolder,
  deleteNote,
  deleteNoteFolder,
  exportNoteTypst,
  finalizeSyncConflicts,
  getPluginConfigPath,
  getSyncStatus,
  getSyncConfigPath,
  getNoteStorageRoot,
  getTagBridgePath,
  initSyncRepository,
  listNoteIdsForTag,
  listNoteTree,
  listTags,
  loadNote,
  listPlugins,
  renameNoteFolder,
  resolveSyncConflict,
  removePlugin,
  runSyncNow,
  runPullOnly,
  runPullThenPush,
  runCommitOnly,
  saveNote,
  setPluginEnabled,
  setAutoSync,
  setSyncRemoteUrl,
  installPlugin,
} from "./lib/noteApi";
import type {
  InstallPluginInput,
  NoteDocument,
  NoteRecord,
  NoteSummary,
  NoteTree,
  PluginEntry,
  SyncStatus,
} from "./types/note";

const tree = ref<NoteTree>({ folders: [], notes: [] });
const selectedNote = ref<NoteRecord | null>(null);
const selectedNoteId = ref<string>("");
const noteStorageRoot = ref<string>("");
const tagBridgePath = ref<string>("");
const syncConfigPath = ref<string>("");
const pluginConfigPath = ref<string>("");
const loadingTree = ref(false);
const busy = ref(false);
const pluginBusy = ref(false);
const notice = ref<string>("");
const error = ref<string>("");
const searchQuery = ref<string>("");
const syncStatus = ref<SyncStatus | null>(null);
const syncBusy = ref(false);
let autoSyncTimer: ReturnType<typeof setInterval> | null = null;
let noticeTimer: ReturnType<typeof setTimeout> | null = null;
let errorTimer: ReturnType<typeof setTimeout> | null = null;
const deleteDialogOpen = ref(false);
const pendingDeleteId = ref("");
const pendingDeleteTitle = ref("");
const pendingDeleteIds = ref<string[]>([]);
const pendingDeleteTitles = ref<string[]>([]);
const folderDeleteDialogOpen = ref(false);
const pendingDeleteFolderPath = ref("");
const pendingDeleteFolderName = ref("");
const pendingDeleteFolderPaths = ref<string[]>([]);
const pendingDeleteFolderNames = ref<string[]>([]);
const activeView = ref<"notes" | "settings" | "search" | "plugins">("notes");
const plugins = ref<PluginEntry[]>([]);
const notesWorkspaceRef = ref<HTMLElement | null>(null);
const sidebarWidth = ref(300);
const sidebarCollapsed = ref(false);
const sidebarResizing = ref(false);
const workspaceWidth = ref(0);
const noteInfoDialogOpen = ref(false);
const exportingTypst = ref(false);

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

const selectedNoteTitle = computed(() => {
  const title = selectedNote.value?.document.title?.trim();
  if (title) {
    return title;
  }
  return selectedNoteId.value ? "Untitled Note" : "No note selected";
});

const selectedNoteAttachmentPaths = computed(() => {
  const document = selectedNote.value?.document;
  if (!document) {
    return [] as string[];
  }

  const paths = document.content
    .map((block) => block.path?.trim() ?? "")
    .filter((path) => path.length > 0);
  return [...new Set(paths)];
});

function resolveNoteDiskFolderPath(noteId: string): string {
  const root = noteStorageRoot.value.trim().replace(/[\\/]+$/, "");
  const relative = noteId.trim().replace(/^[/\\]+/, "");
  if (!root) {
    return relative;
  }
  if (!relative) {
    return root;
  }
  return `${root}/${relative}`;
}

const selectedNoteDiskPath = computed(() => {
  if (!selectedNoteId.value) {
    return "";
  }
  return resolveNoteDiskFolderPath(selectedNoteId.value);
});

const selectedNoteTomlPath = computed(() => {
  const folder = selectedNoteDiskPath.value;
  if (!folder) {
    return "";
  }
  return `${folder}/note.toml`;
});

const notesWorkspaceStyle = computed(() => {
  const clamped = clampSidebarWidth(sidebarWidth.value);
  if (sidebarCollapsed.value) {
    return {
      gridTemplateColumns: "0 minmax(0, 1fr)",
    };
  }

  return {
    gridTemplateColumns: `${clamped}px 7px minmax(0, 1fr)`,
  };
});

async function bootstrap() {
  loadingTree.value = true;
  error.value = "";

  try {
    noteStorageRoot.value = await getNoteStorageRoot();
    tagBridgePath.value = await getTagBridgePath();
    syncConfigPath.value = await getSyncConfigPath();
    pluginConfigPath.value = await getPluginConfigPath();
    await refreshTree();
    await refreshTags();
    await refreshSyncStatus();
    await refreshPlugins();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    loadingTree.value = false;
  }
}

async function refreshPlugins() {
  try {
    plugins.value = await listPlugins();
  } catch (reason) {
    error.value = toErrorMessage(reason);
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

async function refreshSyncStatus() {
  try {
    syncStatus.value = await getSyncStatus();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  }
}

async function selectNote(id: string) {
  if (selectedNoteId.value && selectedNoteId.value !== id) {
    const previousTitle = selectedNote.value?.document.title?.trim() || "";
    const commitMessage = previousTitle ? `Update note ${previousTitle}` : "Update note";
    try {
      const commitStatus = await runCommitOnly(commitMessage);
      if (commitStatus.message && commitStatus.message !== "No changes to commit") {
        notice.value = commitStatus.message;
      }
    } catch {
      // Keep note navigation resilient even if auto-commit fails.
    }
  }

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

function showSelectedNoteInfo() {
  if (!selectedNote.value) {
    error.value = "Select a note first.";
    return;
  }

  noteInfoDialogOpen.value = true;
}

async function handleExportSelectedNoteTypst() {
  if (!selectedNoteId.value) {
    error.value = "Select a note first.";
    return;
  }

  const selectedPath = await open({
    directory: true,
    multiple: false,
    title: "Choose export output folder",
  });
  if (!selectedPath) {
    return;
  }

  const outputPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;

  exportingTypst.value = true;
  error.value = "";

  try {
    const result = await exportNoteTypst(selectedNoteId.value, outputPath || undefined);
    const attachmentLabel = result.attachmentCount === 1 ? "1 attachment" : `${result.attachmentCount} attachments`;
    notice.value = `Exported Typst to '${result.exportFile}' (${attachmentLabel}).`;
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    exportingTypst.value = false;
  }
}

async function handleDeleteFromTree(noteId: string, noteTitle: string) {
  pendingDeleteId.value = noteId;
  pendingDeleteTitle.value = noteTitle;
  pendingDeleteIds.value = [];
  pendingDeleteTitles.value = [];
  pendingDeleteFolderPaths.value = [];
  pendingDeleteFolderNames.value = [];
  deleteDialogOpen.value = true;
}

function handleRenameFolderInTree(folderPath: string, folderName: string) {
  void renameFolderByPath(folderPath, folderName);
}

function handleRenameNoteInTree(noteId: string, noteTitle: string) {
  void renameNoteById(noteId, noteTitle);
}

async function renameFolderByPath(path: string, rawName: string) {
  const nextName = rawName.trim();
  if (!path) {
    return;
  }

  if (!nextName) {
    error.value = "Folder name cannot be empty.";
    return;
  }

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
  }
}

async function renameNoteById(noteId: string, rawTitle: string) {
  const nextTitle = rawTitle.trim();
  if (!noteId) {
    return;
  }

  if (!nextTitle) {
    error.value = "Note title cannot be empty.";
    return;
  }

  const noteSummary = tree.value.notes.find((entry) => entry.id === noteId);
  if (noteSummary && noteSummary.title === nextTitle) {
    return;
  }

  busy.value = true;
  error.value = "";

  try {
    const currentRecord = selectedNoteId.value === noteId
      ? selectedNote.value
      : await loadNote(noteId);

    if (!currentRecord) {
      throw new Error("Note not found.");
    }

    const nextDocument: NoteDocument = {
      ...currentRecord.document,
      title: nextTitle,
    };

    await saveNote(noteId, nextDocument);

    if (selectedNoteId.value === noteId && selectedNote.value) {
      selectedNote.value = {
        ...selectedNote.value,
        document: {
          ...selectedNote.value.document,
          title: nextTitle,
        },
      };
    }

    notice.value = `Renamed note to '${nextTitle}'.`;
    await refreshTree();
    await refreshTags();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    busy.value = false;
  }
}

function handleDeleteFolderInTree(folderPath: string, folderName: string) {
  pendingDeleteFolderPath.value = folderPath;
  pendingDeleteFolderName.value = folderName;
  pendingDeleteIds.value = [];
  pendingDeleteTitles.value = [];
  pendingDeleteFolderPaths.value = [];
  pendingDeleteFolderNames.value = [];
  folderDeleteDialogOpen.value = true;
}

function handleDeleteTreeSelection(payload: {
  noteIds: string[];
  noteTitles: string[];
  folderPaths: string[];
  folderNames: string[];
}) {
  const normalizedNoteIds = [...new Set(payload.noteIds.filter(Boolean))];
  const normalizedFolderPaths = [...new Set(payload.folderPaths.filter(Boolean))];

  const noteTitleById = new Map<string, string>();
  payload.noteIds.forEach((id, index) => {
    if (id) {
      noteTitleById.set(id, payload.noteTitles[index] || "Untitled Note");
    }
  });

  const folderNameByPath = new Map<string, string>();
  payload.folderPaths.forEach((path, index) => {
    if (path) {
      folderNameByPath.set(path, payload.folderNames[index] || path.split("/").pop() || "folder");
    }
  });

  pendingDeleteIds.value = normalizedNoteIds;
  pendingDeleteTitles.value = normalizedNoteIds.map((id) => noteTitleById.get(id) || "Untitled Note");
  pendingDeleteFolderPaths.value = normalizedFolderPaths;
  pendingDeleteFolderNames.value = normalizedFolderPaths.map(
    (path) => folderNameByPath.get(path) || path.split("/").pop() || "folder",
  );

  pendingDeleteId.value = "";
  pendingDeleteTitle.value = "";
  pendingDeleteFolderPath.value = "";
  pendingDeleteFolderName.value = "";

  if (pendingDeleteFolderPaths.value.length > 0) {
    folderDeleteDialogOpen.value = true;
    deleteDialogOpen.value = false;
    return;
  }

  if (pendingDeleteIds.value.length > 0) {
    deleteDialogOpen.value = true;
    folderDeleteDialogOpen.value = false;
  }
}

function cancelDeleteFolder() {
  folderDeleteDialogOpen.value = false;
  pendingDeleteFolderPath.value = "";
  pendingDeleteFolderName.value = "";
  pendingDeleteFolderPaths.value = [];
  pendingDeleteFolderNames.value = [];
  pendingDeleteIds.value = [];
  pendingDeleteTitles.value = [];
}

async function confirmDeleteFolder() {
  const folderPaths = pendingDeleteFolderPaths.value.length
    ? [...pendingDeleteFolderPaths.value]
    : pendingDeleteFolderPath.value
      ? [pendingDeleteFolderPath.value]
      : [];
  const folderNames = pendingDeleteFolderNames.value.length
    ? [...pendingDeleteFolderNames.value]
    : [pendingDeleteFolderName.value || "folder"];
  const selectionNoteIds = [...pendingDeleteIds.value];

  if (folderPaths.length === 0) {
    cancelDeleteFolder();
    return;
  }

  folderDeleteDialogOpen.value = false;
  busy.value = true;
  error.value = "";

  try {
    for (const folderPath of folderPaths) {
      await deleteNoteFolder(folderPath);
      if (selectedNoteId.value && selectedNoteId.value.startsWith(`${folderPath}/`)) {
        selectedNoteId.value = "";
        selectedNote.value = null;
      }
    }

    const remainingNoteIds = selectionNoteIds.filter((noteId) => {
      return !folderPaths.some((folderPath) => noteId.startsWith(`${folderPath}/`));
    });

    for (const noteId of remainingNoteIds) {
      await deleteNote(noteId);
      if (selectedNoteId.value === noteId) {
        selectedNoteId.value = "";
        selectedNote.value = null;
      }
    }

    if (folderPaths.length > 0 && remainingNoteIds.length > 0) {
      notice.value = `Deleted ${folderPaths.length} folders and ${remainingNoteIds.length} notes.`;
    } else if (folderPaths.length === 1) {
      notice.value = `Deleted folder '${folderNames[0] || "folder"}'.`;
    } else {
      notice.value = `Deleted ${folderPaths.length} folders.`;
    }

    await refreshTree();
    await refreshTags();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    busy.value = false;
    pendingDeleteFolderPath.value = "";
    pendingDeleteFolderName.value = "";
    pendingDeleteFolderPaths.value = [];
    pendingDeleteFolderNames.value = [];
    pendingDeleteIds.value = [];
    pendingDeleteTitles.value = [];
  }
}

function cancelDeleteNote() {
  deleteDialogOpen.value = false;
  pendingDeleteId.value = "";
  pendingDeleteTitle.value = "";
  pendingDeleteIds.value = [];
  pendingDeleteTitles.value = [];
}

async function confirmDeleteNote() {
  const noteIds = pendingDeleteIds.value.length
    ? [...pendingDeleteIds.value]
    : pendingDeleteId.value
      ? [pendingDeleteId.value]
      : [];
  const noteTitles = pendingDeleteTitles.value.length
    ? [...pendingDeleteTitles.value]
    : [pendingDeleteTitle.value || "Untitled Note"];

  if (noteIds.length === 0) {
    cancelDeleteNote();
    return;
  }

  deleteDialogOpen.value = false;

  busy.value = true;
  error.value = "";

  try {
    for (const noteId of noteIds) {
      await deleteNote(noteId);

      if (selectedNoteId.value === noteId) {
        selectedNote.value = null;
        selectedNoteId.value = "";
      }
    }

    if (noteIds.length === 1) {
      notice.value = `Deleted '${noteTitles[0] || "Untitled Note"}'.`;
    } else {
      notice.value = `Deleted ${noteIds.length} notes.`;
    }

    await refreshTree();
    await refreshTags();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    busy.value = false;
    pendingDeleteId.value = "";
    pendingDeleteTitle.value = "";
    pendingDeleteIds.value = [];
    pendingDeleteTitles.value = [];
  }
}

async function handleSyncNow() {
  syncBusy.value = true;
  error.value = "";

  try {
    syncStatus.value = await runSyncNow();
    await refreshTree();
    await refreshTags();
    if (syncStatus.value?.message) {
      notice.value = syncStatus.value.message;
    }
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    syncBusy.value = false;
  }
}

async function handleQuickPull() {
  syncBusy.value = true;
  error.value = "";

  try {
    syncStatus.value = await runPullOnly();
    await refreshTree();
    await refreshTags();
    if (syncStatus.value?.message) {
      notice.value = syncStatus.value.message;
    }
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    syncBusy.value = false;
  }
}

async function handleQuickUpload() {
  syncBusy.value = true;
  error.value = "";

  try {
    syncStatus.value = await runPullThenPush();
    await refreshTree();
    await refreshTags();
    if (syncStatus.value?.message) {
      notice.value = syncStatus.value.message;
    }
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    syncBusy.value = false;
  }
}

async function handleInitSync() {
  syncBusy.value = true;
  error.value = "";

  try {
    syncStatus.value = await initSyncRepository();
    notice.value = "Initialized note sync repository.";
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    syncBusy.value = false;
  }
}

async function handleSetRemote(remoteUrl: string) {
  const trimmed = remoteUrl.trim();
  if (!trimmed) {
    error.value = "Remote URL cannot be empty.";
    return;
  }

  syncBusy.value = true;
  error.value = "";

  try {
    syncStatus.value = await setSyncRemoteUrl(trimmed);
    notice.value = `Sync remote set to '${trimmed}'.`;
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    syncBusy.value = false;
  }
}

async function handleSetAutoSync(enabled: boolean, intervalSec: number) {
  syncBusy.value = true;
  error.value = "";

  try {
    syncStatus.value = await setAutoSync(enabled, intervalSec);
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    syncBusy.value = false;
  }
}

async function handleResolveConflict(noteId: string, useLocal: boolean) {
  syncBusy.value = true;
  error.value = "";

  try {
    syncStatus.value = await resolveSyncConflict(noteId, useLocal);
    if (syncStatus.value?.message) {
      notice.value = syncStatus.value.message;
    }
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    syncBusy.value = false;
  }
}

async function handleFinalizeConflicts() {
  syncBusy.value = true;
  error.value = "";

  try {
    syncStatus.value = await finalizeSyncConflicts();
    await refreshTree();
    await refreshTags();
    if (syncStatus.value?.message) {
      notice.value = syncStatus.value.message;
    }
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    syncBusy.value = false;
  }
}

async function handleInstallPlugin(input: InstallPluginInput) {
  pluginBusy.value = true;
  error.value = "";

  try {
    const entry = await installPlugin(input);
    notice.value = `Installed plugin '${entry.name}'.`;
    await refreshPlugins();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    pluginBusy.value = false;
  }
}

async function handleTogglePlugin(pluginId: string, enabled: boolean) {
  pluginBusy.value = true;
  error.value = "";

  try {
    const entry = await setPluginEnabled(pluginId, enabled);
    notice.value = `${entry.name} ${enabled ? "enabled" : "disabled"}.`;
    await refreshPlugins();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    pluginBusy.value = false;
  }
}

async function handleRemovePlugin(pluginId: string) {
  pluginBusy.value = true;
  error.value = "";

  try {
    await removePlugin(pluginId);
    notice.value = `Removed plugin '${pluginId}'.`;
    await refreshPlugins();
  } catch (reason) {
    error.value = toErrorMessage(reason);
  } finally {
    pluginBusy.value = false;
  }
}

function clearAutoSyncTimer() {
  if (autoSyncTimer) {
    clearInterval(autoSyncTimer);
    autoSyncTimer = null;
  }
}

function clearNoticeTimer() {
  if (noticeTimer) {
    clearTimeout(noticeTimer);
    noticeTimer = null;
  }
}

function clearErrorTimer() {
  if (errorTimer) {
    clearTimeout(errorTimer);
    errorTimer = null;
  }
}

watch(notice, (message) => {
  clearNoticeTimer();
  if (!message) {
    return;
  }

  noticeTimer = setTimeout(() => {
    if (notice.value === message) {
      notice.value = "";
    }
  }, 3000);
});

watch(error, (message) => {
  clearErrorTimer();
  if (!message) {
    return;
  }

  errorTimer = setTimeout(() => {
    if (error.value === message) {
      error.value = "";
    }
  }, 3000);
});

watch(
  () => [syncStatus.value?.autoSyncEnabled, syncStatus.value?.autoSyncIntervalSec],
  ([enabled, intervalSec]) => {
    clearAutoSyncTimer();

    if (!enabled) {
      return;
    }

    const interval = Math.max(Number(intervalSec) || 180, 30);
    autoSyncTimer = setInterval(() => {
      if (!syncBusy.value) {
        void handleSyncNow();
      }
    }, interval * 1000);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  clearAutoSyncTimer();
  clearNoticeTimer();
  clearErrorTimer();
  onSidebarResizeEnd();
  window.removeEventListener("resize", syncSidebarConstraints);
});

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
  syncSidebarConstraints();
  window.addEventListener("resize", syncSidebarConstraints);
  void bootstrap();
});

function openNoteFromSearch(noteId: string) {
  void selectNote(noteId);
  activeView.value = "notes";
  sidebarCollapsed.value = false;
}

function onNotesRailClick() {
  if (activeView.value === "notes") {
    sidebarCollapsed.value = !sidebarCollapsed.value;
    if (!sidebarCollapsed.value) {
      syncSidebarConstraints();
    }
    return;
  }

  activeView.value = "notes";
  sidebarCollapsed.value = false;
  syncSidebarConstraints();
}

function clampSidebarWidth(raw: number): number {
  const totalWidth = workspaceWidth.value || notesWorkspaceRef.value?.getBoundingClientRect().width || 0;
  if (!totalWidth) {
    return Math.max(180, Math.min(520, raw));
  }

  const minWidth = 180;
  const minEditorWidth = 320;
  const maxWidth = Math.min(520, Math.max(minWidth, totalWidth - minEditorWidth - 7));
  return Math.max(minWidth, Math.min(maxWidth, raw));
}

function syncSidebarConstraints() {
  if (!notesWorkspaceRef.value) {
    return;
  }

  workspaceWidth.value = notesWorkspaceRef.value.getBoundingClientRect().width;
  sidebarWidth.value = clampSidebarWidth(sidebarWidth.value);
}

function onSidebarResizeMove(event: MouseEvent) {
  if (!sidebarResizing.value || !notesWorkspaceRef.value) {
    return;
  }

  const bounds = notesWorkspaceRef.value.getBoundingClientRect();
  workspaceWidth.value = bounds.width;
  const nextWidth = event.clientX - bounds.left;
  sidebarWidth.value = clampSidebarWidth(nextWidth);
}

function onSidebarResizeEnd() {
  if (!sidebarResizing.value) {
    return;
  }

  sidebarResizing.value = false;
  window.removeEventListener("mousemove", onSidebarResizeMove);
  window.removeEventListener("mouseup", onSidebarResizeEnd);
}

function startSidebarResize(event: MouseEvent) {
  if (sidebarCollapsed.value) {
    return;
  }

  event.preventDefault();
  sidebarResizing.value = true;
  window.addEventListener("mousemove", onSidebarResizeMove);
  window.addEventListener("mouseup", onSidebarResizeEnd);
}

</script>

<template>
  <v-app>
    <v-main class="app-shell">
      <div class="app-layout">
        <aside class="activity-rail">
          <button type="button" class="rail-btn" :class="{ active: activeView === 'notes' }" @click="onNotesRailClick"
            title="Notes">
            <v-icon icon="mdi-notebook-outline" size="20" />
            <span>Notes</span>
          </button>
          <button type="button" class="rail-btn" :class="{ active: activeView === 'search' }"
            @click="activeView = 'search'" title="Search">
            <v-icon icon="mdi-magnify" size="20" />
            <span>Search</span>
          </button>
          <button type="button" class="rail-btn" :class="{ active: activeView === 'settings' }"
            @click="activeView = 'settings'" title="Settings">
            <v-icon icon="mdi-cog-outline" size="20" />
            <span>Settings</span>
          </button>
          <button type="button" class="rail-btn" :class="{ active: activeView === 'plugins' }"
            @click="activeView = 'plugins'" title="Plugins">
            <v-icon icon="mdi-puzzle-outline" size="20" />
            <span>Plugins</span>
          </button>

          <div class="rail-bottom-actions">
            <button type="button" class="rail-btn rail-utility" :disabled="syncBusy || busy" @click="handleQuickPull"
              title="Git Pull">
              <v-icon icon="mdi-download" size="18" />
              <span>Pull</span>
            </button>
            <button type="button" class="rail-btn rail-utility" :disabled="syncBusy || busy" @click="handleQuickUpload"
              title="Git Pull then Push">
              <v-icon icon="mdi-upload" size="18" />
              <span>Upload</span>
            </button>
          </div>
        </aside>

        <div class="page-wrap">

          <header v-if="activeView === 'notes'" class="page-note-banner">
            <div class="page-note-banner-spacer" aria-hidden="true" />
            <h2 class="page-note-banner-title" :title="selectedNoteTitle">{{ selectedNoteTitle }}</h2>
            <div class="page-note-banner-actions">
              <v-btn size="small" variant="text" prepend-icon="mdi-information-outline" :disabled="!selectedNote"
                @click="showSelectedNoteInfo">
                Show info
              </v-btn>
              <v-btn size="small" color="primary" variant="flat" prepend-icon="mdi-export" :disabled="!selectedNoteId"
                :loading="exportingTypst" @click="handleExportSelectedNoteTypst">
                Export
              </v-btn>
            </div>
          </header>

          <div class="notice-stack" aria-live="polite">
            <v-alert v-if="notice" type="success" variant="tonal" class="notice-row" closable @click:close="notice = ''">
              {{ notice }}
            </v-alert>

            <v-alert v-if="error" type="error" variant="tonal" class="notice-row" closable @click:close="error = ''">
              {{ error }}
            </v-alert>
          </div>

          <section v-if="activeView === 'notes'" ref="notesWorkspaceRef" class="workspace-grid"
            :class="{ collapsed: sidebarCollapsed }" :style="notesWorkspaceStyle">
            <aside class="sidebar-column">
              <NoteTreePanel :folders="foldersForTree" :notes="notesForTree" :selected-note-id="selectedNoteId"
                :loading="loadingTree || busy" :show-empty-folders="showEmptyFolders" @select-note="selectNote"
                @create-note-in-folder="handleCreateNoteInFolder" @create-folder-in-folder="handleCreateFolderInFolder"
                @rename-folder-in-tree="handleRenameFolderInTree" @delete-folder-in-tree="handleDeleteFolderInTree"
                @rename-note-in-tree="handleRenameNoteInTree" @delete-note-in-tree="handleDeleteFromTree"
                @delete-tree-selection="handleDeleteTreeSelection" />
            </aside>

            <div class="sidebar-resizer" :class="{ dragging: sidebarResizing }" @mousedown="startSidebarResize" />

            <article class="editor-column">
              <NoteEditorPanel :note="selectedNote" :tag-options="tagOptions" @change="handleSave" />
            </article>
          </section>

          <section v-else-if="activeView === 'settings'" class="settings-view">
            <SyncPanel :status="syncStatus" :busy="syncBusy || busy" :config-path="syncConfigPath"
              @refresh="refreshSyncStatus" @init="handleInitSync" @set-remote="handleSetRemote"
              @sync-now="handleSyncNow" @set-auto-sync="handleSetAutoSync" @resolve-conflict="handleResolveConflict"
              @finalize-conflicts="handleFinalizeConflicts" />
          </section>

          <section v-else-if="activeView === 'plugins'" class="settings-view">
            <PluginsPanel :plugins="plugins" :busy="pluginBusy || busy" :config-path="pluginConfigPath"
              @refresh="refreshPlugins" @install="handleInstallPlugin" @toggle="handleTogglePlugin"
              @remove="handleRemovePlugin" />
          </section>

          <SearchPage v-else :tags="tags" @open-note="openNoteFromSearch" />

          <footer class="footer" style="display: none;">
            <span>Storage root: <code>{{ noteStorageRoot || 'loading...' }}</code></span>
            <span>Tag bridge: <code>{{ tagBridgePath || 'loading...' }}</code></span>
            <span>Sync config: <code>{{ syncConfigPath || 'loading...' }}</code></span>
            <span>Plugin config: <code>{{ pluginConfigPath || 'loading...' }}</code></span>
          </footer>
        </div>
      </div>
    </v-main>

    <v-dialog v-model="deleteDialogOpen" max-width="460">
      <v-card>
        <v-card-title>
          {{ pendingDeleteIds.length > 1 ? `Delete ${pendingDeleteIds.length} notes?` : "Delete note?" }}
        </v-card-title>
        <v-card-text>
          <template v-if="pendingDeleteIds.length > 1">
            Delete {{ pendingDeleteIds.length }} selected notes and their assets? This cannot be undone.
          </template>
          <template v-else>
            Delete '{{ pendingDeleteTitle || "Untitled Note" }}' and its assets? This cannot be undone.
          </template>
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="cancelDeleteNote">Cancel</v-btn>
          <v-btn color="error" variant="flat" :loading="busy" @click="confirmDeleteNote">Delete</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="folderDeleteDialogOpen" max-width="500">
      <v-card>
        <v-card-title>
          {{ pendingDeleteFolderPaths.length > 1 ?
            `Delete ${pendingDeleteFolderPaths.length} folders?` : "Delete folder?"
          }}
        </v-card-title>
        <v-card-text>
          <template v-if="pendingDeleteFolderPaths.length > 1">
            Delete {{ pendingDeleteFolderPaths.length }} selected folders and all notes inside them? This cannot be
            undone.
          </template>
          <template v-else>
            Delete folder '{{ pendingDeleteFolderName || "folder" }}' and all notes inside? This cannot be undone.
          </template>
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="cancelDeleteFolder">Cancel</v-btn>
          <v-btn color="error" variant="flat" :loading="busy" @click="confirmDeleteFolder">Delete</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="noteInfoDialogOpen" max-width="560">
      <v-card>
        <v-card-title>Note info</v-card-title>
        <v-card-text>
          <p><strong>Title:</strong> {{ selectedNote?.document.title || "Untitled Note" }}</p>
          <p><strong>Path:</strong> <code>{{ selectedNote?.id || "-" }}</code></p>
          <p><strong>Disk folder:</strong> <code>{{ selectedNoteDiskPath || "-" }}</code></p>
          <p><strong>TOML file:</strong> <code>{{ selectedNoteTomlPath || "-" }}</code></p>
          <p><strong>Folder:</strong> <code>{{ selectedNote?.folder || "root" }}</code></p>
          <p><strong>Date:</strong> {{ selectedNote?.document.date || "-" }}</p>
          <p><strong>Type:</strong> {{ selectedNote?.document.type || "-" }}</p>
          <p><strong>Tags:</strong> {{ selectedNote?.document.tags.length ? selectedNote?.document.tags.join(", ") :
            "(none)" }}</p>
          <p><strong>Blocks:</strong> {{ selectedNote?.document.content.length || 0 }}</p>
          <p><strong>Attachments:</strong> {{ selectedNoteAttachmentPaths.length }}</p>
          <p v-if="selectedNoteAttachmentPaths.length > 0" class="attachments-preview">
            {{ selectedNoteAttachmentPaths.join(", ") }}
          </p>
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="noteInfoDialogOpen = false">Close</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

  </v-app>
</template>

<style scoped>
.app-shell {
  padding: 0;
}

.app-layout {
  display: grid;
  grid-template-columns: 78px minmax(0, 1fr);
  height: 100vh;
}

.activity-rail {
  border-right: 1px solid var(--fox-border);
  background: color-mix(in srgb, var(--fox-surface) 92%, #0f1522 8%);
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  padding: 0.55rem 0.4rem;
}

.rail-bottom-actions {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  padding-top: 0.4rem;
  border-top: 1px solid color-mix(in srgb, var(--fox-border) 80%, transparent 20%);
}

.rail-utility {
  min-height: 50px;
}

.rail-btn {
  border: 1px solid transparent;
  background: transparent;
  color: var(--fox-text-muted);
  border-radius: 10px;
  min-height: 56px;
  padding: 0.32rem 0.2rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.2rem;
  font-size: 0.72rem;
  font-weight: 600;
  cursor: pointer;
}

.rail-btn:hover {
  color: var(--fox-text-body);
  background: color-mix(in srgb, var(--fox-chip) 78%, transparent 22%);
}

.rail-btn.active {
  border-color: var(--fox-border);
  color: var(--fox-text-strong);
  background: color-mix(in srgb, var(--fox-chip) 70%, #314261 30%);
}

.page-wrap {
  width: 100%;
  margin: 0;
  height: 100vh;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: rise-in 0.38s ease-out;
}

.notice-stack {
  position: absolute;
  top: 4.1rem;
  right: 0.7rem;
  z-index: 22;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 0.45rem;
  pointer-events: none;
}

.notice-row {
  width: fit-content;
  max-width: min(560px, calc(100vw - 7.2rem));
  border-radius: 10px;
  backdrop-filter: blur(4px);
  pointer-events: auto;
}

.notice-row :deep(.v-alert__content) {
  white-space: normal;
}

.page-note-banner {
  position: sticky;
  top: 0;
  z-index: 12;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  gap: 0.55rem;
  border-bottom: 1px solid var(--fox-border);
  padding: 0.52rem 0.95rem;
  margin: 0 -0.02rem 0.2rem;
  background: color-mix(in srgb, #101522 88%, var(--fox-surface) 12%);
}

.page-note-banner-spacer {
  min-height: 1px;
}

.page-note-banner-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--fox-text-strong);
  max-width: min(58vw, 620px);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: center;
}

.page-note-banner-actions {
  justify-self: end;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}

.workspace-grid {
  margin-top: 0;
  display: grid;
  gap: 0;
  grid-template-columns: 300px 7px minmax(0, 1fr);
  flex: 1;
  min-height: 0;
  min-width: 0;
}

.workspace-grid.collapsed {
  grid-template-columns: 0 minmax(0, 1fr);
}

.settings-view {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0.75rem 0.9rem;
}


.sidebar-column {
  min-width: 0;
  border-right: 1px solid var(--fox-border);
  padding: 0.2rem 0.25rem 0.45rem 0.4rem;
  overflow: hidden;
  opacity: 1;
  transition: opacity 140ms ease;
}

.workspace-grid.collapsed .sidebar-column {
  border-right: 0;
  padding: 0;
  opacity: 0;
}

.sidebar-resizer {
  cursor: col-resize;
  background: transparent;
  position: relative;
}

.sidebar-resizer::before {
  content: "";
  position: absolute;
  inset: 0;
  width: 1px;
  margin: 0 auto;
  background: color-mix(in srgb, var(--fox-border) 78%, transparent 22%);
}

.sidebar-resizer:hover::before,
.sidebar-resizer.dragging::before {
  width: 2px;
  background: color-mix(in srgb, var(--fox-text-muted) 70%, #5d7ab0 30%);
}

.sidebar-resizer.hidden {
  display: none;
}

.workspace-grid.collapsed .sidebar-resizer {
  display: none;
}

.editor-column {
  min-width: 0;
  width: 100%;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  padding: 0.7rem 1.15rem 0.7rem 1rem;
  overflow-x: hidden;
}

.attachments-preview {
  margin-top: 0.4rem;
  color: var(--fox-text-muted);
  font-size: 0.82rem;
  word-break: break-word;
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

@media (max-width: 760px) {
  .app-layout {
    grid-template-columns: 62px minmax(0, 1fr);
  }

  .activity-rail {
    flex-direction: column;
    border-right: 1px solid var(--fox-border);
    border-bottom: 0;
    padding: 0.4rem 0.2rem;
  }

  .rail-btn {
    min-height: 50px;
    font-size: 0.66rem;
    padding: 0.24rem 0.12rem;
  }

  .sidebar-column {
    padding: 0.2rem 0.2rem 0.3rem 0.3rem;
  }

  .editor-column {
    padding: 0.45rem 0.6rem 0.7rem;
  }

  .notice-stack {
    top: 3.8rem;
    right: 0.45rem;
  }

  .notice-row {
    max-width: calc(100vw - 4.8rem);
  }

  .page-note-banner {
    grid-template-columns: 1fr;
    justify-items: stretch;
    padding: 0.45rem 0.5rem;
  }

  .page-note-banner-spacer {
    display: none;
  }

  .page-note-banner-title {
    max-width: 100%;
    text-align: left;
  }

  .page-note-banner-actions {
    justify-self: start;
  }

  .footer {
    font-size: 0.74rem;
    padding: 0.35rem 0.5rem;
  }

  .sidebar-resizer {
    display: none;
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

<style>
body.fox-marquee-active {
  -webkit-user-select: none;
  user-select: none;
}
</style>
