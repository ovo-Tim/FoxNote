<script setup lang="ts">
import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { Tldraw, TldrawImage, serializeTldrawJson } from "tldraw";
import { loadNoteAttachment, saveNoteAttachment } from "../../lib/noteApi";
import "tldraw/tldraw.css";

const props = defineProps<{
  noteId: string;
  path: string;
  editing?: boolean;
  width?: number;
  height?: number;
}>();

const emit = defineEmits<{
  focus: [];
  updatePath: [path: string];
  updateSize: [width: number, height: number];
}>();

const mountRef = ref<HTMLDivElement | null>(null);
const previewMountRef = ref<HTMLDivElement | null>(null);
const cardRef = ref<HTMLElement | null>(null);
const reactRoot = ref<Root | null>(null);
const previewRoot = ref<Root | null>(null);
const editorRef = ref<any>(null);
const snapshotState = ref<unknown | null>(null);
const snapshotVersion = ref(0);
const loading = ref(false);
const saving = ref(false);
const windowFill = ref(false);
const resizing = ref(false);
const resizeStart = ref({ x: 0, y: 0, width: 0, height: 0 });
const suppressFocusClick = ref(false);

const hasPath = computed(() => props.path.trim().length > 0);

const frameWidth = computed(() => {
  const requested = Math.round(Number(props.width) || 0);
  if (requested >= 180) {
    return Math.min(2200, requested);
  }
  return 960;
});

const frameHeight = computed(() => {
  const requested = Math.round(Number(props.height) || 0);
  if (requested >= 140) {
    return Math.min(1800, requested);
  }
  return 540;
});

const frameStyle = computed(() => ({
  width: `${frameWidth.value}px`,
  height: `${frameHeight.value}px`,
}));

const editorFrameStyle = computed(() => ({
  width: `${frameWidth.value}px`,
}));

function bumpSnapshot(snapshot: unknown | null) {
  snapshotState.value = snapshot;
  snapshotVersion.value += 1;
}

async function loadSnapshotFromAttachment() {
  const noteId = props.noteId.trim();
  const path = props.path.trim();

  if (!noteId || !path) {
    bumpSnapshot(null);
    return;
  }

  loading.value = true;
  try {
    const payload = await loadNoteAttachment(noteId, path, "application/vnd.tldraw+json");
    const bytes = Uint8Array.from(payload.bytes ?? []);
    const text = new TextDecoder().decode(bytes);
    const parsed = JSON.parse(text);
    bumpSnapshot(parsed);
  } catch (reason) {
    console.error("[CanvasBlock] failed to load canvas attachment", reason);
    bumpSnapshot(null);
  } finally {
    loading.value = false;
  }
}

function mountReactEditor() {
  if (!props.editing || !mountRef.value || !reactRoot.value) {
    return;
  }

  const key = `${props.noteId}:${props.path}:${snapshotVersion.value}`;

  reactRoot.value.render(
    createElement(Tldraw as any, {
      key,
      inferDarkMode: true,
      autoFocus: true,
      snapshot: snapshotState.value ?? undefined,
      onMount: (editor: any) => {
        editorRef.value = editor;
      },
    }),
  );
}

function clearReactEditor() {
  editorRef.value = null;
  reactRoot.value?.render(createElement("div"));
}

function mountPreview() {
  if (props.editing || !previewMountRef.value || !previewRoot.value) {
    return;
  }

  if (!snapshotState.value) {
    previewRoot.value.render(createElement("div"));
    return;
  }

  previewRoot.value.render(
    createElement(TldrawImage as any, {
      snapshot: snapshotState.value,
      format: "svg",
      background: false,
      darkMode: true,
      padding: 12,
      scale: 1,
    }),
  );
}

function ensureReactRoot() {
  if (!mountRef.value) {
    reactRoot.value?.unmount();
    reactRoot.value = null;
    return;
  }

  if (!reactRoot.value) {
    reactRoot.value = createRoot(mountRef.value);
  }
}

function ensurePreviewRoot() {
  if (!previewMountRef.value) {
    previewRoot.value?.unmount();
    previewRoot.value = null;
    return;
  }

  if (!previewRoot.value) {
    previewRoot.value = createRoot(previewMountRef.value);
  }
}

async function saveCanvas() {
  if (!props.noteId.trim() || !editorRef.value) {
    return;
  }

  if (saving.value) {
    return;
  }

  saving.value = true;

  try {
    let json = "";
    const storeSnapshot = editorRef.value?.store?.getStoreSnapshot?.();
    if (storeSnapshot) {
      json = JSON.stringify(storeSnapshot);
    } else {
      json = await serializeTldrawJson(editorRef.value as any);
    }

    try {
      bumpSnapshot(JSON.parse(json));
    } catch {
      // Ignore preview refresh when snapshot parsing fails.
    }

    const bytes = Array.from(new TextEncoder().encode(json));
    const path = await saveNoteAttachment(
      props.noteId,
      "application/vnd.tldraw+json",
      bytes,
      "canvas",
      "tldr",
    );
    emit("updatePath", path);
  } catch (reason) {
    console.error("[CanvasBlock] failed to save canvas attachment", reason);
  } finally {
    saving.value = false;
  }
}

function toggleWindowFill() {
  windowFill.value = !windowFill.value;
}

function onResizePointerMove(event: PointerEvent) {
  if (!resizing.value) {
    return;
  }

  const deltaX = event.clientX - resizeStart.value.x;
  const deltaY = event.clientY - resizeStart.value.y;
  const nextWidth = Math.max(180, Math.min(2200, resizeStart.value.width + deltaX));
  const nextHeight = Math.max(140, Math.min(1800, resizeStart.value.height + deltaY));
  emit("updateSize", Math.round(nextWidth), Math.round(nextHeight));
}

function stopResize() {
  if (!resizing.value) {
    return;
  }

  resizing.value = false;
  window.removeEventListener("pointermove", onResizePointerMove);
  window.removeEventListener("pointerup", stopResize);
}

function onResizeHandlePointerDown(event: PointerEvent) {
  if (event.button !== 0) {
    return;
  }

  event.preventDefault();
  event.stopPropagation();
  suppressFocusClick.value = true;
  resizing.value = true;
  resizeStart.value = {
    x: event.clientX,
    y: event.clientY,
    width: frameWidth.value,
    height: frameHeight.value,
  };
  window.addEventListener("pointermove", onResizePointerMove);
  window.addEventListener("pointerup", stopResize);
}

function onCardClick(event: MouseEvent) {
  if (suppressFocusClick.value) {
    suppressFocusClick.value = false;
    event.stopPropagation();
    return;
  }

  emit("focus");
}

watch(
  () => [props.noteId, props.path],
  () => {
    void loadSnapshotFromAttachment();
  },
  { immediate: true },
);

watch(
  () => props.editing,
  async (editing, previousEditing) => {
    if (!editing) {
      windowFill.value = false;
      if (previousEditing) {
        await saveCanvas();
      }
      clearReactEditor();
      ensurePreviewRoot();
      mountPreview();
      return;
    }

    await nextTick();
    ensureReactRoot();
    mountReactEditor();
  },
  { immediate: true },
);

watch(snapshotVersion, () => {
  if (props.editing) {
    mountReactEditor();
    return;
  }

  mountPreview();
});

watch(
  () => mountRef.value,
  (element) => {
    ensureReactRoot();

    if (!element) {
      return;
    }

    if (props.editing) {
      mountReactEditor();
    }
  },
  { immediate: true },
);

watch(
  () => previewMountRef.value,
  (element) => {
    ensurePreviewRoot();

    if (!element) {
      return;
    }

    if (!props.editing) {
      mountPreview();
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  stopResize();
  editorRef.value = null;
  previewRoot.value?.unmount();
  previewRoot.value = null;
  reactRoot.value?.unmount();
  reactRoot.value = null;
});
</script>

<template>
  <section ref="cardRef" class="canvas-card tldraw-zone"
    :class="{ editing: Boolean(editing), 'window-fill': windowFill, resizing }" @click="onCardClick">
    <header v-if="editing" class="canvas-header">
      <span class="canvas-label">Canvas</span>
      <div class="canvas-actions">
        <v-btn size="x-small" variant="text" :icon="windowFill ? 'mdi-window-restore' : 'mdi-window-maximize'"
          :disabled="!editing" :title="windowFill ? 'Restore size' : 'Fill window'" @click.stop="toggleWindowFill" />
        <v-btn size="x-small" variant="flat" color="primary" prepend-icon="mdi-content-save-outline"
          :disabled="!editing || !noteId || loading" :loading="saving" @click.stop="saveCanvas">
          Save
        </v-btn>
      </div>
    </header>

    <p v-if="editing && hasPath" class="canvas-path">{{ path }}</p>

    <div v-if="editing" ref="mountRef" class="canvas-editor" :style="windowFill ? undefined : editorFrameStyle" />
    <div v-else class="canvas-preview-wrap">
      <div v-if="hasPath" ref="previewMountRef" class="canvas-preview" :style="frameStyle" />
      <div v-else class="canvas-hint">
        Click to start drawing
      </div>
    </div>
    <button
      v-if="!windowFill"
      type="button"
      class="block-resize-handle"
      title="Resize block"
      @pointerdown="onResizeHandlePointerDown"
      @mousedown.stop.prevent
      @click.stop
    />
  </section>
</template>

<style scoped>
.canvas-card {
  position: relative;
  width: 100%;
  border-radius: 12px;
  background: color-mix(in srgb, var(--fox-surface) 92%, #0f1422 8%);
  padding: 0.55rem;
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}

.canvas-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.canvas-label {
  color: var(--fox-text-strong);
  font-size: 0.9rem;
  font-weight: 600;
}

.canvas-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}

.canvas-path,
.canvas-hint {
  margin: 0;
  font-size: 0.8rem;
  color: var(--fox-text-muted);
  word-break: break-all;
}

.canvas-preview-wrap {
  width: 100%;
}

.canvas-preview {
  max-width: 100%;
  min-height: 140px;
  border-radius: 10px;
  overflow: hidden;
  background:
    linear-gradient(45deg, color-mix(in srgb, var(--fox-surface) 90%, #171d2a 10%) 25%, transparent 25%),
    linear-gradient(-45deg, color-mix(in srgb, var(--fox-surface) 90%, #171d2a 10%) 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, color-mix(in srgb, var(--fox-surface) 90%, #171d2a 10%) 75%),
    linear-gradient(-45deg, transparent 75%, color-mix(in srgb, var(--fox-surface) 90%, #171d2a 10%) 75%);
  background-size: 22px 22px;
  background-position: 0 0, 0 11px, 11px -11px, -11px 0;
}

.canvas-preview :deep(svg) {
  width: 100%;
  height: auto;
  display: block;
}

.canvas-editor {
  max-width: 100%;
  height: min(64vh, 620px);
  border-radius: 10px;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--fox-border) 80%, transparent 20%);
}

.canvas-card.window-fill {
  position: fixed;
  inset: 0;
  z-index: 45;
  width: 100%;
  height: 100%;
  border-radius: 0;
  padding: 0.75rem;
  background: color-mix(in srgb, #0c1220 92%, var(--fox-surface) 8%);
}

.canvas-card.window-fill .canvas-editor {
  width: 100%;
  height: calc(100vh - 86px);
}

.block-resize-handle {
  position: absolute;
  right: 8px;
  bottom: 8px;
  width: 14px;
  height: 14px;
  border: 1px solid color-mix(in srgb, var(--fox-text-muted) 76%, transparent 24%);
  border-radius: 3px;
  background: color-mix(in srgb, var(--fox-surface) 82%, #1b263a 18%);
  cursor: nwse-resize;
}

.block-resize-handle:hover {
  border-color: color-mix(in srgb, var(--fox-text-strong) 75%, transparent 25%);
}
</style>
