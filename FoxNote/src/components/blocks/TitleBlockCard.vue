<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { renderTypstToSvgWithTheme } from "../../lib/typstPreview";

const props = defineProps<{
  modelValue: string;
  level: number;
  folded: boolean;
  summary: string;
  editing: boolean;
}>();

const emit = defineEmits<{
  focus: [];
  blur: [];
  updateModelValue: [value: string];
  updateLevel: [level: number];
  updateFolded: [folded: boolean];
  updateSummary: [summary: string];
}>();

const localSummary = ref(props.summary || "");
const previewSvg = ref("");
const previewError = ref("");
const prefersDark = ref(false);
const previewHostRef = ref<HTMLElement | null>(null);
const previewPageWidth = ref<string | undefined>(undefined);
let renderTicket = 0;
let mediaQuery: MediaQueryList | null = null;
let previewResizeObserver: ResizeObserver | null = null;

const headingTag = computed(() => {
  const level = Math.min(6, Math.max(1, Number(props.level) || 1));
  return `h${level}`;
});

watch(
  () => props.summary,
  (value) => {
    localSummary.value = value || "";
  },
);

const displayTitle = computed(() => props.modelValue.trim() || "Untitled section");

const titlePreviewSource = computed(() => {
  const level = Math.min(6, Math.max(1, Number(props.level) || 1));
  return `${"=".repeat(level)} ${displayTitle.value}`;
});

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
    return false;
  }

  const widthPx = Math.max(0, host.clientWidth - 8);
  if (widthPx <= 0) {
    return false;
  }

  const widthPt = Math.max(120, Math.round(widthPx * 0.95));
  const nextWidth = `${widthPt}pt`;
  if (previewPageWidth.value === nextWidth) {
    return false;
  }

  previewPageWidth.value = nextWidth;
  return true;
}

function bindPreviewResizeObserver() {
  previewResizeObserver?.disconnect();
  previewResizeObserver = null;

  if (typeof ResizeObserver === "undefined" || !previewHostRef.value) {
    return;
  }

  previewResizeObserver = new ResizeObserver(() => {
    syncPreviewWidth();
  });
  previewResizeObserver.observe(previewHostRef.value);
}

async function renderPreview() {
  if (props.editing || (props.folded && props.summary.trim())) {
    previewSvg.value = "";
    previewError.value = "";
    return;
  }

  const ticket = ++renderTicket;
  try {
    const svg = await renderTypstToSvgWithTheme(titlePreviewSource.value, {
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
  }
}

function onSummaryBlur() {
  emit("updateSummary", localSummary.value);
}

function onLevelInput(rawValue: string | number) {
  const parsed = Number(rawValue);
  if (!Number.isFinite(parsed)) {
    return;
  }
  const next = Math.min(6, Math.max(1, Math.round(parsed)));
  emit("updateLevel", next);
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
  void renderPreview();
});

onBeforeUnmount(() => {
  mediaQuery?.removeEventListener("change", syncDarkMode);
  mediaQuery = null;
  previewResizeObserver?.disconnect();
  previewResizeObserver = null;
});

watch(
  () => [props.modelValue, props.level, props.editing, props.folded, props.summary, prefersDark.value, previewPageWidth.value],
  () => {
    void renderPreview();
  },
);
</script>

<template>
  <section class="title-card" :class="{ editing }" @click="!editing && emit('focus')">
    <div class="title-row">
      <button type="button" class="fold-toggle" :title="folded ? 'Expand section' : 'Fold section'"
        @click.stop="emit('updateFolded', !folded)">
        <v-icon :icon="folded ? 'mdi-chevron-right' : 'mdi-chevron-down'" size="16" />
      </button>

      <component v-if="editing" :is="headingTag" class="title-heading">{{ displayTitle }}</component>

      <div v-else ref="previewHostRef" class="title-preview-wrap">
        <div v-if="previewSvg" class="title-preview typst-content" v-html="previewSvg" />
        <component v-else :is="headingTag" class="title-heading title-heading-fallback">{{ displayTitle }}</component>
      </div>

      <div class="title-level-chip" title="Title level">H{{ Math.min(6, Math.max(1, Number(level) || 1)) }}</div>
    </div>

    <template v-if="editing">
      <div class="title-editor-grid">
        <v-text-field :model-value="modelValue" density="compact" variant="solo-filled" hide-details label="Title"
          class="title-input" @update:model-value="(value) => emit('updateModelValue', String(value ?? ''))" />

        <v-text-field :model-value="String(Math.min(6, Math.max(1, Number(level) || 1)))" density="compact"
          variant="solo-filled" hide-details type="number" min="1" max="6" label="Level" class="level-input"
          @update:model-value="onLevelInput" />
      </div>

      <v-textarea v-model="localSummary" rows="2" max-rows="5" auto-grow density="compact" variant="solo-filled"
        hide-details class="summary-input" label="Summary when folded"
        placeholder="Optional summary shown when section is folded" @blur="onSummaryBlur" />
    </template>

    <p v-else-if="folded && summary.trim()" class="title-summary">{{ summary }}</p>
    <p v-else-if="previewError" class="title-preview-error">{{ previewError }}</p>
  </section>
</template>

<style scoped>
.title-card {
  border-radius: 10px;
  background: color-mix(in srgb, var(--fox-surface) 90%, black 10%);
  padding: 0.4rem 0.5rem;
  width: 100%;
}

.title-card.editing {
  border-color: color-mix(in srgb, var(--fox-primary) 40%, transparent 60%);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--fox-primary) 22%, transparent 78%);
}

.title-row {
  display: flex;
  align-items: center;
  gap: 0.45rem;
}

.fold-toggle {
  background: color-mix(in srgb, var(--fox-surface) 88%, #182033 12%);
  color: var(--fox-text-body);
  width: 24px;
  height: 24px;
  border-radius: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.fold-toggle:hover {
  background: color-mix(in srgb, var(--fox-chip) 70%, transparent 30%);
}

.title-heading {
  margin: 0;
  flex: 1;
  min-width: 0;
  color: var(--fox-text-strong);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.title-heading-fallback {
  width: 100%;
}

.title-preview-wrap {
  flex: 1;
  min-width: 0;
}

.title-preview {
  width: 100%;
}

.title-preview :deep(svg) {
  display: block;
  width: 100%;
  height: auto;
}

.title-level-chip {
  border: 1px solid color-mix(in srgb, var(--fox-border) 78%, transparent 22%);
  border-radius: 999px;
  padding: 0.06rem 0.42rem;
  font-size: 0.72rem;
  color: var(--fox-text-muted);
}

.title-editor-grid {
  margin-top: 0.45rem;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 96px;
  gap: 0.35rem;
}

.summary-input {
  margin-top: 0.38rem;
}

.title-summary {
  margin: 0.42rem 0 0;
  color: var(--fox-text-muted);
  font-size: 0.9rem;
  white-space: pre-wrap;
  word-break: break-word;
}

.title-preview-error {
  margin: 0.42rem 0 0;
  color: #fda4af;
  font-size: 0.82rem;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
