<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
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
const previewHostRef = ref<HTMLElement | null>(null);
const previewPageWidth = ref<string | undefined>(undefined);
let renderTicket = 0;

let mediaQuery: MediaQueryList | null = null;
let previewResizeObserver: ResizeObserver | null = null;

function applyInlineWrap(value: string, start: number, end: number, left: string, right = left): {
  nextValue: string;
  nextStart: number;
  nextEnd: number;
} {
  const safeStart = Math.max(0, Math.min(value.length, start));
  const safeEnd = Math.max(safeStart, Math.min(value.length, end));
  const selected = value.slice(safeStart, safeEnd);

  if (safeStart !== safeEnd) {
    const nextValue = `${value.slice(0, safeStart)}${left}${selected}${right}${value.slice(safeEnd)}`;
    return {
      nextValue,
      nextStart: safeStart + left.length,
      nextEnd: safeEnd + left.length,
    };
  }

  const nextValue = `${value.slice(0, safeStart)}${left}${right}${value.slice(safeStart)}`;
  const caret = safeStart + left.length;
  return {
    nextValue,
    nextStart: caret,
    nextEnd: caret,
  };
}

function applyUnderlineWrap(value: string, start: number, end: number): {
  nextValue: string;
  nextStart: number;
  nextEnd: number;
} {
  const safeStart = Math.max(0, Math.min(value.length, start));
  const safeEnd = Math.max(safeStart, Math.min(value.length, end));
  const selected = value.slice(safeStart, safeEnd);
  const prefix = "#underline[";
  const suffix = "]";

  if (safeStart !== safeEnd) {
    const nextValue = `${value.slice(0, safeStart)}${prefix}${selected}${suffix}${value.slice(safeEnd)}`;
    return {
      nextValue,
      nextStart: safeStart + prefix.length,
      nextEnd: safeEnd + prefix.length,
    };
  }

  const nextValue = `${value.slice(0, safeStart)}${prefix}${suffix}${value.slice(safeStart)}`;
  const caret = safeStart + prefix.length;
  return {
    nextValue,
    nextStart: caret,
    nextEnd: caret,
  };
}

function applyFunctionWrap(value: string, start: number, end: number, name: string): {
  nextValue: string;
  nextStart: number;
  nextEnd: number;
} {
  const safeStart = Math.max(0, Math.min(value.length, start));
  const safeEnd = Math.max(safeStart, Math.min(value.length, end));
  const selected = value.slice(safeStart, safeEnd);
  const prefix = `#${name}[`;
  const suffix = "]";

  if (safeStart !== safeEnd) {
    const nextValue = `${value.slice(0, safeStart)}${prefix}${selected}${suffix}${value.slice(safeEnd)}`;
    return {
      nextValue,
      nextStart: safeStart + prefix.length,
      nextEnd: safeEnd + prefix.length,
    };
  }

  const placeholder = `Your ${name} text`;
  const nextValue = `${value.slice(0, safeStart)}${prefix}${placeholder}${suffix}${value.slice(safeStart)}`;
  return {
    nextValue,
    nextStart: safeStart + prefix.length,
    nextEnd: safeStart + prefix.length + placeholder.length,
  };
}

function onEditorKeydown(event: KeyboardEvent) {
  if (!(event.metaKey || event.ctrlKey)) {
    return;
  }

  const target = event.target;
  if (!(target instanceof HTMLTextAreaElement) && !(target instanceof HTMLInputElement)) {
    return;
  }

  const shortcut = event.key.toLowerCase();
  let result: { nextValue: string; nextStart: number; nextEnd: number } | null = null;
  if (shortcut === "b") {
    event.preventDefault();
    result = applyInlineWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0, "*", "*");
  } else if (shortcut === "i") {
    event.preventDefault();
    result = applyInlineWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0, "_", "_");
  } else if (shortcut === "u") {
    event.preventDefault();
    result = applyUnderlineWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0);
  } else if (event.shiftKey && event.code === "Digit1") {
    event.preventDefault();
    result = applyFunctionWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0, "note");
  } else if (event.shiftKey && event.code === "Digit2") {
    event.preventDefault();
    result = applyFunctionWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0, "tip");
  } else if (event.shiftKey && event.code === "Digit3") {
    event.preventDefault();
    result = applyFunctionWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0, "important");
  } else if (event.shiftKey && event.code === "Digit4") {
    event.preventDefault();
    result = applyFunctionWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0, "warning");
  } else if (event.shiftKey && event.code === "Digit5") {
    event.preventDefault();
    result = applyFunctionWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0, "caution");
  } else if (event.shiftKey && event.code === "Digit6") {
    event.preventDefault();
    result = applyFunctionWrap(props.modelValue, target.selectionStart ?? 0, target.selectionEnd ?? 0, "todo");
  }

  if (!result) {
    return;
  }

  emit("updateModelValue", result.nextValue);
  void nextTick(() => {
    target.focus();
    target.setSelectionRange(result.nextStart, result.nextEnd);
  });
}

function stripSvgScripts(svg: string): string {
  return svg.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, "");
}

function syncDarkMode() {
  if (!mediaQuery) {
    return;
  }
  prefersDark.value = mediaQuery.matches;
}

function syncPreviewWidth() {
  const host = previewHostRef.value;
  if (!host) {
    return;
  }

  const widthPx = Math.max(0, host.clientWidth - 8);
  if (widthPx <= 0) {
    return;
  }

  const widthPt = Math.max(120, Math.round(widthPx * 0.75));
  previewPageWidth.value = `${widthPt}pt`;
}

function bindPreviewResizeObserver() {
  previewResizeObserver?.disconnect();
  previewResizeObserver = null;

  if (typeof ResizeObserver === "undefined" || !previewHostRef.value) {
    return;
  }

  previewResizeObserver = new ResizeObserver(() => {
    syncPreviewWidth();
    void renderPreview(props.modelValue);
  });
  previewResizeObserver.observe(previewHostRef.value);
}

onMounted(() => {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return;
  }

  mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  syncDarkMode();
  mediaQuery.addEventListener("change", syncDarkMode);
  syncPreviewWidth();
  bindPreviewResizeObserver();
});

onBeforeUnmount(() => {
  mediaQuery?.removeEventListener("change", syncDarkMode);
  mediaQuery = null;
  previewResizeObserver?.disconnect();
  previewResizeObserver = null;
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
      pageWidth: previewPageWidth.value,
    });
    if (ticket !== renderTicket) {
      return;
    }

    previewSvg.value = stripSvgScripts(svg);
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

watch(
  () => previewHostRef.value,
  () => {
    void nextTick(() => {
      syncPreviewWidth();
      bindPreviewResizeObserver();
      void renderPreview(props.modelValue);
    });
  },
);

watch(
  () => props.editing,
  () => {
    void nextTick(() => {
      syncPreviewWidth();
      bindPreviewResizeObserver();
      void renderPreview(props.modelValue);
    });
  },
);

function onInput(value: string) {
  emit("updateModelValue", value);
}
</script>

<template>
  <section class="typst-card" :class="{ editing }" @click="!editing && emit('focus')">
    <template v-if="editing">
      <div class="editor-grid">
        <div class="pane">
          <v-textarea :model-value="modelValue" rows="3" max-rows="16" auto-grow hide-details density="compact"
            variant="solo-filled" class="code-input" @update:model-value="(value) => onInput(String(value ?? ''))"
            placeholder="Input typst code here..."
            title="Shortcuts: Cmd/Ctrl+B bold, Cmd/Ctrl+I italic, Cmd/Ctrl+U underline, Cmd/Ctrl+Shift+1 note, +2 tip, +3 important, +4 warning, +5 caution, +6 todo"
            @blur="emit('blur')" @keydown="onEditorKeydown" />
        </div>

        <div ref="previewHostRef" class="pane preview-pane">
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
      <div ref="previewHostRef" class="preview-only">
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
  background: transparent;
  padding: 0.06rem 0.1rem;
  cursor: pointer;
  width: 100%;
  min-width: 0;
}

.typst-card.editing {
  cursor: default;
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--fox-primary) 20%, transparent 80%);
}

.editor-grid {
  display: grid;
  gap: 0.5rem;
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
  height: 100%;
}

.preview-pane {
  border: 1px solid color-mix(in srgb, var(--fox-border) 85%, transparent 15%);
  border-radius: 8px;
  padding: 0.18rem;
  background: color-mix(in srgb, var(--fox-surface) 84%, black 16%);
}

.preview-only {
  min-height: 0;
  width: 100%;
  min-width: 0;
}

.preview-sheet {
  /* border-radius: 8px; */
  background: transparent;
  border: 0;
  box-shadow: none;
  overflow: visible;
  max-height: none;
  width: 100%;
  min-width: 0;
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
  width: 100%;
  min-width: 0;
  overflow-wrap: anywhere;
  word-break: break-word;
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
