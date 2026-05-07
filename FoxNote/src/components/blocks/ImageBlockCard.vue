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
}>();

const emit = defineEmits<{
  focus: [];
  updatePath: [path: string];
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

const altText = computed(() => props.path || "image attachment");

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
  revokeImageUrl();
});
</script>

<template>
  <section class="image-card" :class="{ 'editing-mode': editing }" @click="emit('focus')">
    <p v-if="loading" class="image-empty">Loading image...</p>
    <div v-else-if="editing && tools.length > 0 && settings" class="editor-wrap image-paint-zone">
      <VpEditor
        v-model:history="history"
        v-model:settings="settings"
        :tools="tools"
        :width="imageWidth"
        :height="imageHeight"
        class="vp-editor"
        @save="onEditorSave"
      />
      <p v-if="saving" class="image-empty">Saving edited image...</p>
      <p v-if="editorError" class="image-empty error">{{ editorError }}</p>
    </div>
    <img v-else-if="imageUrl" class="image-preview" :src="imageUrl" :alt="altText" />
    <p v-else-if="loadError" class="image-empty">{{ loadError }}</p>
    <p v-else class="image-empty">Image attachment missing.</p>
    <p class="image-path" v-if="path">{{ path }}</p>
  </section>
</template>

<style scoped>
.image-card {
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
  max-height: min(62vh, 580px);
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
  border-radius: 8px;
  overflow: hidden;
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
</style>
