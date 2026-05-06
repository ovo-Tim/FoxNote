<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { renderTypstToSvgWithTheme } from "../../lib/typstPreview";

const props = defineProps<{
  modelValue: string;
  editing: boolean;
}>();

const emit = defineEmits<{
  updateModelValue: [value: string];
  focus: [];
  blur: [];
}>();

const previewSvg = ref("");
const previewError = ref("");
const rendering = ref(false);
const prefersDark = ref(false);
let renderTicket = 0;

let mediaQuery: MediaQueryList | null = null;

function syncDarkMode() {
  if (!mediaQuery) {
    return;
  }
  prefersDark.value = mediaQuery.matches;
}

onMounted(() => {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return;
  }

  mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  syncDarkMode();
  mediaQuery.addEventListener("change", syncDarkMode);
});

onBeforeUnmount(() => {
  mediaQuery?.removeEventListener("change", syncDarkMode);
  mediaQuery = null;
});

const previewText = computed(() => {
  if (props.modelValue.trim().length === 0) {
    return "Click to edit this Typst block";
  }
  return props.modelValue;
});

async function renderPreview(source: string) {
  const ticket = ++renderTicket;

  if (!source.trim()) {
    previewSvg.value = "";
    previewError.value = "";
    rendering.value = false;
    return;
  }

  rendering.value = true;
  try {
    const svg = await renderTypstToSvgWithTheme(source, {
      darkMode: prefersDark.value,
      pageWidth: "960pt",
    });
    if (ticket !== renderTicket) {
      return;
    }
    previewSvg.value = svg;
    previewError.value = "";
  } catch (reason) {
    if (ticket !== renderTicket) {
      return;
    }
    previewSvg.value = "";
    previewError.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    if (ticket === renderTicket) {
      rendering.value = false;
    }
  }
}

watch(
  () => props.modelValue,
  (value) => {
    void renderPreview(value);
  },
  { immediate: true },
);

watch(prefersDark, () => {
  void renderPreview(props.modelValue);
});

function onInput(value: string) {
  emit("updateModelValue", value);
}
</script>

<template>
  <section class="typst-card" :class="{ editing }" @click="!editing && emit('focus')">
    <template v-if="editing">
      <div class="editor-grid">
        <div class="pane">
          <p class="pane-label">Typst</p>
          <v-textarea :model-value="modelValue" rows="3" max-rows="16" auto-grow hide-details density="compact"
            variant="solo-filled" class="code-input" @update:model-value="(value) => onInput(String(value ?? ''))"
            @blur="emit('blur')" />
        </div>

        <div class="pane preview-pane">
          <p class="pane-label">Preview</p>
          <div v-if="previewSvg" class="preview-sheet" :class="{ 'is-dark': prefersDark }">
            <div class="preview-svg" v-html="previewSvg" />
          </div>
          <pre v-else-if="previewError" class="preview-error">{{ previewError }}</pre>
          <pre v-else-if="rendering" class="preview-text">Rendering...</pre>
          <pre v-else class="preview-text">{{ previewText }}</pre>
        </div>
      </div>
    </template>

    <template v-else>
      <div class="preview-only">
        <div v-if="previewSvg" class="preview-sheet" :class="{ 'is-dark': prefersDark }">
          <div class="preview-svg" v-html="previewSvg" />
        </div>
        <pre v-else-if="previewError" class="preview-error">{{ previewError }}</pre>
        <pre v-else class="preview-text">{{ previewText }}</pre>
      </div>
    </template>
  </section>
</template>

<style scoped>
.typst-card {
  border: 0;
  border-radius: 8px;
  background: var(--fox-surface);
  padding: 0.2rem;
  cursor: pointer;
}

.typst-card.editing {
  cursor: default;
  border-color: color-mix(in srgb, var(--fox-primary) 38%, var(--fox-border) 62%);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--fox-primary) 28%, transparent 72%);
}

.editor-grid {
  display: grid;
  gap: 0.6rem;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
}

.pane {
  min-width: 0;
}

.pane-label {
  margin: 0 0 0.4rem;
  font-size: 0.8rem;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--fox-text-muted);
}

.code-input {
  font-family: "Iosevka", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
}

.preview-pane {
  border: 1px solid var(--fox-border);
  border-radius: 8px;
  padding: 0.2rem;
  background: color-mix(in srgb, var(--fox-surface) 84%, black 16%);
}

.preview-only {
  min-height: 0;
}

.preview-sheet {
  border-radius: 8px;
  background: transparent;
  border: 0;
  box-shadow: none;
  overflow: auto;
  max-height: min(70vh, 820px);
}

.preview-text {
  margin: 0;
  font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
  font-size: 1rem;
  color: var(--fox-text-body);
  white-space: pre-wrap;
  word-break: break-word;
}

.preview-error {
  margin: 0;
  color: #ffb4b4;
  white-space: pre-wrap;
  word-break: break-word;
}

.preview-svg :deep(svg) {
  max-width: 100%;
  min-width: 0;
  height: auto;
  display: block;
  background: transparent;
}

.preview-svg {
  padding: 0;
}

.preview-sheet.is-dark .preview-svg :deep(svg) {
  color-scheme: dark;
}

.preview-sheet.is-dark .preview-svg :deep(svg text),
.preview-sheet.is-dark .preview-svg :deep(svg tspan) {
  fill: #edf2ff !important;
}

.preview-sheet.is-dark .preview-svg :deep(svg path[fill="#000"]),
.preview-sheet.is-dark .preview-svg :deep(svg path[fill="#000000"]),
.preview-sheet.is-dark .preview-svg :deep(svg polygon[fill="#000"]),
.preview-sheet.is-dark .preview-svg :deep(svg polygon[fill="#000000"]) {
  fill: #edf2ff !important;
}

@media (max-width: 980px) {
  .editor-grid {
    grid-template-columns: 1fr;
  }
}
</style>
