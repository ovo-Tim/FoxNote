<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import {
  VpEditor,
  createSettings,
  exportPng,
  useArrow,
  useBackground,
  useCrop,
  useEllipse,
  useEraser,
  useFreehand,
  useLine,
  useMove,
  useRectangle,
  useTextarea,
} from "vue-paint";
import "vue-paint/themes/default.css";
import { loadNoteImageAttachment, saveNoteImageAttachment } from "../../lib/noteApi";

const props = defineProps<{
  noteId: string;
  path: string;
  editing: boolean;
  width?: number;
  height?: number;
}>();

const emit = defineEmits<{
  focus: [];
  updatePath: [path: string];
  updateSize: [width: number, height: number];
}>();

const imageUrl = ref("");
const sourceBlob = ref<Blob | null>(null);
const loading = ref(false);
const saving = ref(false);
const loadError = ref("");
const editorError = ref("");
const imageWidth = ref(1280);
const imageHeight = ref(720);
const tools = ref<any[]>([]);
const history = ref<any[]>([]);
const settings = ref<any>(null);
const resizing = ref(false);
const resizeStart = ref({ x: 0, y: 0, width: 0, height: 0 });
const suppressFocusClick = ref(false);

const altText = computed(() => props.path || "image attachment");

const frameWidth = computed(() => {
  const requested = Math.round(Number(props.width) || 0);
  if (requested >= 140) {
    return Math.min(2048, requested);
  }
  return Math.max(180, Math.min(2048, imageWidth.value));
});

const frameHeight = computed(() => {
  const requested = Math.round(Number(props.height) || 0);
  if (requested >= 120) {
    return Math.min(2048, requested);
  }
  return Math.max(140, Math.min(2048, imageHeight.value));
});

const frameStyle = computed(() => ({
  width: `${frameWidth.value}px`,
  height: `${frameHeight.value}px`,
}));

const editorFrameStyle = computed(() => ({
  width: `${frameWidth.value}px`,
}));

function revokeImageUrl() {
  if (!imageUrl.value) {
    return;
  }
  URL.revokeObjectURL(imageUrl.value);
  imageUrl.value = "";
}

async function refreshImage() {
  revokeImageUrl();
  sourceBlob.value = null;
  loadError.value = "";
  editorError.value = "";

  if (!props.noteId || !props.path.trim()) {
    return;
  }

  loading.value = true;
  try {
    const payload = await loadNoteImageAttachment(props.noteId, props.path);
    const bytes = new Uint8Array(payload.bytes);
    const blob = new Blob([bytes], { type: payload.mimeType || "image/png" });
    sourceBlob.value = blob;
    imageUrl.value = URL.createObjectURL(blob);
    await updateImageDimensions(imageUrl.value);
    prepareEditor();
  } catch (reason) {
    loadError.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    loading.value = false;
  }
}

function prepareEditor() {
  if (!sourceBlob.value) {
    tools.value = [];
    history.value = [];
    settings.value = null;
    return;
  }

  const nextTools = [
    useBackground({ blob: sourceBlob.value }),
    useMove(),
    useCrop(),
    useFreehand(),
    useLine(),
    useArrow(),
    useRectangle(),
    useEllipse(),
    useTextarea(),
    useEraser(),
  ];
  tools.value = nextTools;
  history.value = [];
  settings.value = createSettings(nextTools, {
    color: "#ff5a5f",
    thickness: 4,
  });
}

async function updateImageDimensions(url: string) {
  await new Promise<void>((resolve) => {
    const image = new Image();
    image.onload = () => {
      const width = image.naturalWidth || 1280;
      const height = image.naturalHeight || 720;
      imageWidth.value = Math.max(64, Math.min(2048, width));
      imageHeight.value = Math.max(64, Math.min(2048, height));
      resolve();
    };
    image.onerror = () => {
      imageWidth.value = 1280;
      imageHeight.value = 720;
      resolve();
    };
    image.src = url;
  });
}

async function onEditorSave(params: any) {
  if (!props.noteId || saving.value) {
    return;
  }

  saving.value = true;
  editorError.value = "";

  try {
    const pngDataUrl = await exportPng(params);
    const response = await fetch(pngDataUrl);
    const bytes = Array.from(new Uint8Array(await response.arrayBuffer()));
    const nextPath = await saveNoteImageAttachment(props.noteId, "image/png", bytes);
    emit("updatePath", nextPath);
  } catch (reason) {
    editorError.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    saving.value = false;
  }
}

function onResizePointerMove(event: PointerEvent) {
  if (!resizing.value) {
    return;
  }

  const deltaX = event.clientX - resizeStart.value.x;
  const deltaY = event.clientY - resizeStart.value.y;
  const nextWidth = Math.max(140, Math.min(2048, resizeStart.value.width + deltaX));
  const nextHeight = Math.max(120, Math.min(2048, resizeStart.value.height + deltaY));
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
    void refreshImage();
  },
  { immediate: true },
);

watch(
  () => props.editing,
  (editing) => {
    if (editing) {
      prepareEditor();
    }
  },
);

onBeforeUnmount(() => {
  stopResize();
  revokeImageUrl();
});
</script>

<template>
  <section class="image-card" :class="{ 'editing-mode': editing, resizing }" @click="onCardClick">
    <p v-if="loading" class="image-empty">Loading image...</p>
    <div v-else-if="editing && tools.length > 0 && settings" class="editor-wrap image-paint-zone" :style="editorFrameStyle">
      <VpEditor
        v-model:history="history"
        v-model:settings="settings"
        :tools="tools"
        :width="frameWidth"
        :height="frameHeight"
        class="vp-editor"
        @save="onEditorSave"
      />
      <p v-if="saving" class="image-empty">Saving edited image...</p>
      <p v-if="editorError" class="image-empty error">{{ editorError }}</p>
    </div>
    <img v-else-if="imageUrl" class="image-preview" :style="frameStyle" :src="imageUrl" :alt="altText" />
    <p v-else-if="loadError" class="image-empty">{{ loadError }}</p>
    <p v-else class="image-empty">Image attachment missing.</p>
    <p class="image-path" v-if="path">{{ path }}</p>
    <button
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
.image-card {
  position: relative;
  border: 1px solid color-mix(in srgb, var(--fox-border) 85%, transparent 15%);
  border-radius: 10px;
  background: color-mix(in srgb, var(--fox-surface) 92%, black 8%);
  padding: 0.42rem;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.image-preview {
  max-width: 100%;
  object-fit: contain;
  border-radius: 8px;
  display: block;
}

.image-empty {
  margin: 0;
  color: var(--fox-text-muted);
}

.image-empty.error {
  color: #ff8f8f;
}

.editor-wrap {
  max-width: 100%;
  border-radius: 8px;
  overflow: visible;
  border: 1px solid color-mix(in srgb, var(--fox-border) 78%, transparent 22%);
}

.vp-editor {
  width: 100%;
  max-width: 100%;
}

:deep(.vp-editor) {
  max-width: 100%;
}

:deep(.vp-main) {
  max-width: 100%;
}

:deep(.vp-toolbar) {
  border-radius: 0;
}

:deep(.vp-toolbar) {
  margin-top: 0.4rem;
  justify-content: flex-start;
  gap: 0.35rem 0.8rem;
}

:deep(.vp-toolbar button),
:deep(.vp-toolbar input[type='color']) {
  background: color-mix(in srgb, var(--fox-surface) 86%, #131b2a 14%);
  border: 1px solid color-mix(in srgb, var(--fox-border) 82%, transparent 18%);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.28);
  color: var(--fox-text-body);
}

:deep(.vp-toolbar button:hover),
:deep(.vp-toolbar button.active),
:deep(.vp-toolbar input[type='color']:hover) {
  background: color-mix(in srgb, var(--fox-chip) 72%, #1f2a3e 28%);
  border-color: color-mix(in srgb, var(--fox-text-muted) 48%, transparent 52%);
  box-shadow: 0 3px 10px rgba(0, 0, 0, 0.34);
}

:deep(.vp-toolbar svg) {
  fill: currentColor;
}

.image-path {
  margin: 0;
  color: var(--fox-text-muted);
  font-size: 0.78rem;
  font-family: "Iosevka", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  word-break: break-word;
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
