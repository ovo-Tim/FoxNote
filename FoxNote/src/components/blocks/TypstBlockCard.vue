<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { renderTypstToSvgWithTheme } from "../../lib/typstPreview";
import { shouldIgnoreBlockFocusClick } from "../../editor/interactionTargets";
import { openPreviewLinkUrl, resolvePreviewLinkUrl } from "../../editor/previewLinks";
import SpellcheckPanel from "../SpellcheckPanel.vue";
import type { SpellcheckIssue } from "../../editor/spellcheck";
import { useSpellcheckField } from "../../editor/spellcheck";
import {
  buildSpellcheckHighlightSegments,
  computeSpellcheckPopoverPlacement,
  type SpellcheckHighlightSegment,
} from "../../editor/spellcheckHighlight";

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
const textareaRef = ref<HTMLTextAreaElement | null>(null);
const textareaShellRef = ref<HTMLElement | null>(null);
const previewPageWidth = ref<string | undefined>(undefined);
const spellcheckPanelRef = ref<InstanceType<typeof SpellcheckPanel> | null>(null);
const activeSpellIssueId = ref<string | null>(null);
const spellPopoverPosition = ref<{ left: number; top: number } | null>(null);
const spellPopoverPlacement = ref<"top" | "bottom">("bottom");
const spellMirrorScrollTop = ref(0);
const spellMirrorScrollLeft = ref(0);
const spellMirrorOffsetTop = ref(0);
const spellMirrorOffsetLeft = ref(0);
const spellMirrorWidth = ref(0);
const spellMirrorHeight = ref(0);
const spellMirrorPaddingTop = ref(12);
const spellMirrorPaddingRight = ref(12);
const spellMirrorPaddingBottom = ref(12);
const spellMirrorPaddingLeft = ref(12);
const spellMirrorFontSize = ref("0.95rem");
const spellMirrorLineHeight = ref("1.5");
const spellMirrorFontFamily = ref(
  '"Iosevka", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
);
const spellMirrorFontWeight = ref("400");
const spellMirrorLetterSpacing = ref("normal");
const spellMirrorTabSize = ref("4");
const spellIssueSpanMap = new Map<string, HTMLElement>();
let renderTicket = 0;

const MAX_PREVIEW_WIDTH_REM = 40;

let mediaQuery: MediaQueryList | null = null;
let previewResizeObserver: ResizeObserver | null = null;
let previewResizeDebounceTimer: ReturnType<typeof setTimeout> | null = null;
let textareaResizeObserver: ResizeObserver | null = null;
let boundTextareaElement: HTMLTextAreaElement | null = null;
let suppressNextEditorBlur = false;

const {
  issues: spellIssues,
  loading: spellLoading,
  error: spellError,
  applySuggestion: applySpellSuggestion,
} = useSpellcheckField({
  source: computed(() => props.modelValue),
  mode: "typst",
  enabled: computed(() => props.editing),
  onApplied: (nextText) => {
    emit("updateModelValue", nextText);
  },
});

const spellHighlightSegments = computed<SpellcheckHighlightSegment[]>(() =>
  buildSpellcheckHighlightSegments(props.modelValue, spellIssues.value),
);

const activeSpellIssue = computed<SpellcheckIssue | null>(() => {
  if (!activeSpellIssueId.value) {
    return null;
  }
  return spellIssues.value.find((issue) => issue.id === activeSpellIssueId.value) ?? null;
});

const spellPanelIssues = computed(() => (activeSpellIssue.value ? [activeSpellIssue.value] : []));
const spellPanelVisible = computed(
  () => props.editing && !!textareaRef.value && (Boolean(activeSpellIssue.value) || Boolean(spellError.value)),
);
const spellHighlightLayerVisible = computed(
  () => props.editing && spellMirrorWidth.value > 0 && spellMirrorHeight.value > 0,
);
const spellHighlightLayerStyle = computed(() => ({
  top: `${spellMirrorOffsetTop.value}px`,
  left: `${spellMirrorOffsetLeft.value}px`,
  width: `${spellMirrorWidth.value}px`,
  height: `${spellMirrorHeight.value}px`,
}));
const spellMirrorStyle = computed(() => ({
  padding: `${spellMirrorPaddingTop.value}px ${spellMirrorPaddingRight.value}px ${spellMirrorPaddingBottom.value}px ${spellMirrorPaddingLeft.value}px`,
  fontSize: spellMirrorFontSize.value,
  lineHeight: spellMirrorLineHeight.value,
  fontFamily: spellMirrorFontFamily.value,
  fontWeight: spellMirrorFontWeight.value,
  letterSpacing: spellMirrorLetterSpacing.value,
  tabSize: spellMirrorTabSize.value,
  transform: `translate(${-spellMirrorScrollLeft.value}px, ${-spellMirrorScrollTop.value}px)`,
}));

watch(
  spellIssues,
  (issues) => {
    if (!issues.length) {
      activeSpellIssueId.value = null;
      spellPopoverPosition.value = null;
      return;
    }

    if (activeSpellIssueId.value && !issues.some((issue) => issue.id === activeSpellIssueId.value)) {
      activeSpellIssueId.value = null;
      spellPopoverPosition.value = null;
    }

    void nextTick(() => {
      updateSpellPopoverPosition();
    });
  },
  { immediate: true },
);

watch(
  () => props.editing,
  (editing) => {
    if (!editing) {
      cleanupTextareaBindings();
      activeSpellIssueId.value = null;
      spellPopoverPosition.value = null;
      return;
    }

    void nextTick(() => {
      bindTextareaElement();
      syncSpellMirrorMetrics();
      syncSpellMirrorScroll();
      updateSpellPopoverPosition();
    });
  },
);

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

function setTextareaElement(element: Element | null) {
  textareaRef.value = element instanceof HTMLTextAreaElement ? element : null;
  syncSpellMirrorMetrics();
}

function resetSpellMirrorMetrics() {
  spellMirrorOffsetTop.value = 0;
  spellMirrorOffsetLeft.value = 0;
  spellMirrorWidth.value = 0;
  spellMirrorHeight.value = 0;
  spellMirrorScrollTop.value = 0;
  spellMirrorScrollLeft.value = 0;
}

function cleanupTextareaBindings() {
  textareaResizeObserver?.disconnect();
  textareaResizeObserver = null;

  if (boundTextareaElement) {
    boundTextareaElement.removeEventListener("scroll", syncSpellMirrorScroll);
  }
  boundTextareaElement = null;
  textareaRef.value = null;
  spellIssueSpanMap.clear();
  resetSpellMirrorMetrics();
}

function getTextareaElement(): HTMLTextAreaElement | null {
  const shell = textareaShellRef.value;
  if (
    !textareaRef.value ||
    !textareaRef.value.isConnected ||
    (shell instanceof HTMLElement && !shell.contains(textareaRef.value))
  ) {
    textareaRef.value = null;
  }

  if (!textareaRef.value && shell) {
    setTextareaElement(shell.querySelector("textarea"));
  }
  return textareaRef.value;
}

function bindTextareaElement() {
  const textarea = getTextareaElement();
  if (!textarea) {
    cleanupTextareaBindings();
    return;
  }

  if (boundTextareaElement === textarea) {
    return;
  }

  cleanupTextareaBindings();
  boundTextareaElement = textarea;
  boundTextareaElement.addEventListener("scroll", syncSpellMirrorScroll, { passive: true });

  if (typeof ResizeObserver !== "undefined") {
    textareaResizeObserver = new ResizeObserver(() => {
      syncSpellMirrorMetrics();
      syncSpellMirrorScroll();
      updateSpellPopoverPosition();
    });
    textareaResizeObserver.observe(boundTextareaElement);
  }

  syncSpellMirrorMetrics();
  syncSpellMirrorScroll();
}

function setSpellIssueSpanRef(issueId: string | undefined, element: unknown) {
  if (!issueId) {
    return;
  }

  if (!(element instanceof HTMLElement)) {
    spellIssueSpanMap.delete(issueId);
    return;
  }

  spellIssueSpanMap.set(issueId, element);
}

function syncSpellMirrorMetrics() {
  const textarea = getTextareaElement();
  const shell = textareaShellRef.value;
  if (!textarea || !shell || typeof window === "undefined") {
    resetSpellMirrorMetrics();
    return;
  }

  const styles = window.getComputedStyle(textarea);
  const textareaRect = textarea.getBoundingClientRect();
  const shellRect = shell.getBoundingClientRect();

  if (textarea.clientWidth <= 0 || textarea.clientHeight <= 0 || shellRect.width <= 0 || shellRect.height <= 0) {
    resetSpellMirrorMetrics();
    return;
  }

  spellMirrorOffsetTop.value = textareaRect.top - shellRect.top;
  spellMirrorOffsetLeft.value = textareaRect.left - shellRect.left;
  spellMirrorWidth.value = textarea.clientWidth;
  spellMirrorHeight.value = textarea.clientHeight;
  spellMirrorPaddingTop.value = Number.parseFloat(styles.paddingTop) || 12;
  spellMirrorPaddingRight.value = Number.parseFloat(styles.paddingRight) || 12;
  spellMirrorPaddingBottom.value = Number.parseFloat(styles.paddingBottom) || 12;
  spellMirrorPaddingLeft.value = Number.parseFloat(styles.paddingLeft) || 12;
  spellMirrorFontSize.value = styles.fontSize;
  spellMirrorLineHeight.value = styles.lineHeight;
  spellMirrorFontFamily.value = styles.fontFamily;
  spellMirrorFontWeight.value = styles.fontWeight;
  spellMirrorLetterSpacing.value = styles.letterSpacing;
  spellMirrorTabSize.value = styles.tabSize;
}

function syncSpellMirrorScroll() {
  const textarea = getTextareaElement();
  if (!textarea) {
    return;
  }

  spellMirrorScrollTop.value = textarea.scrollTop;
  spellMirrorScrollLeft.value = textarea.scrollLeft;
  updateSpellPopoverPosition();
}

function updateSpellPopoverPosition() {
  const activeIssue = activeSpellIssue.value;
  if (!activeIssue || typeof window === "undefined") {
    spellPopoverPosition.value = null;
    return;
  }

  const panelElement = spellcheckPanelRef.value?.$el;
  if (!(panelElement instanceof HTMLElement)) {
    return;
  }

  const anchorElement = spellIssueSpanMap.get(activeIssue.id);
  if (!anchorElement) {
    spellPopoverPosition.value = null;
    return;
  }

  const rect = anchorElement.getBoundingClientRect();
  const viewport = { width: window.innerWidth, height: window.innerHeight };
  const panelRect = panelElement.getBoundingClientRect();

  const placement = computeSpellcheckPopoverPlacement(rect, viewport, {
    width: panelRect.width || 340,
    height: panelRect.height || 220,
  });

  spellPopoverPlacement.value = placement.placement;
  spellPopoverPosition.value = { left: placement.left, top: placement.top };
}

function onSpellHighlightPointerDown(issueId: string) {
  activeSpellIssueId.value = issueId;
  void nextTick(() => {
    updateSpellPopoverPosition();
  });
}

function onSpellPanelApply(issueId: string, suggestionId: string) {
  void applySpellSuggestion(issueId, suggestionId).then(() => {
    activeSpellIssueId.value = null;
    void nextTick(() => {
      updateSpellPopoverPosition();
    });
  });
}

function closeSpellPopover() {
  activeSpellIssueId.value = null;
  spellPopoverPosition.value = null;
}

function onSpellPanelPointerDown() {
  suppressNextEditorBlur = true;
  if (typeof window !== "undefined") {
    window.requestAnimationFrame(() => {
      suppressNextEditorBlur = false;
    });
  }
}

function onEditorBlur(event: FocusEvent) {
  const panelElement = spellcheckPanelRef.value?.$el;
  const relatedTarget = event.relatedTarget instanceof Node ? event.relatedTarget : null;

  if (
    suppressNextEditorBlur ||
    (panelElement instanceof HTMLElement && ((relatedTarget && panelElement.contains(relatedTarget)) || panelElement.contains(document.activeElement)))
  ) {
    void nextTick(() => {
      getTextareaElement()?.focus();
    });
    return;
  }

  emit("blur");
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
    return false;
  }

  const rootFontSize =
    typeof window !== "undefined"
      ? Number.parseFloat(window.getComputedStyle(document.documentElement).fontSize) || 16
      : 16;
  const maxWidthPx = MAX_PREVIEW_WIDTH_REM * rootFontSize;
  const widthPx = Math.max(0, Math.min(host.clientWidth - 8, maxWidthPx));
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

function clearPreviewResizeDebounceTimer() {
  if (!previewResizeDebounceTimer) {
    return;
  }
  clearTimeout(previewResizeDebounceTimer);
  previewResizeDebounceTimer = null;
}

function bindPreviewResizeObserver() {
  previewResizeObserver?.disconnect();
  previewResizeObserver = null;

  if (typeof ResizeObserver === "undefined" || !previewHostRef.value) {
    return;
  }

  previewResizeObserver = new ResizeObserver(() => {
    const widthChanged = syncPreviewWidth();
    if (!widthChanged) {
      return;
    }

    clearPreviewResizeDebounceTimer();
    previewResizeDebounceTimer = setTimeout(() => {
      previewResizeDebounceTimer = null;
      void renderPreview(props.modelValue);
    }, 120);
  });
  previewResizeObserver.observe(previewHostRef.value);
}

onMounted(() => {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return;
  }

  mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  syncDarkMode();
  bindTextareaElement();
  syncSpellMirrorMetrics();
  syncSpellMirrorScroll();
  mediaQuery.addEventListener("change", syncDarkMode);
  syncPreviewWidth();
  bindPreviewResizeObserver();
  window.addEventListener("resize", updateSpellPopoverPosition);
  document.addEventListener("pointerdown", onDocumentPointerDown, true);
});

onBeforeUnmount(() => {
  mediaQuery?.removeEventListener("change", syncDarkMode);
  mediaQuery = null;
  previewResizeObserver?.disconnect();
  previewResizeObserver = null;
  clearPreviewResizeDebounceTimer();
  cleanupTextareaBindings();
  window.removeEventListener("resize", updateSpellPopoverPosition);
  document.removeEventListener("pointerdown", onDocumentPointerDown, true);
});

function onDocumentPointerDown(event: PointerEvent) {
  if (!activeSpellIssue.value) {
    return;
  }

  const target = event.target instanceof Node ? event.target : null;
  const panelElement = spellcheckPanelRef.value?.$el;
  const issueElement = activeSpellIssueId.value ? spellIssueSpanMap.get(activeSpellIssueId.value) : null;
  if (
    (panelElement instanceof HTMLElement && target && panelElement.contains(target)) ||
    (issueElement && target && issueElement.contains(target))
  ) {
    return;
  }

  closeSpellPopover();
}

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
  (editing) => {
    if (!editing) {
      return;
    }

    void nextTick(() => {
      bindTextareaElement();
      syncSpellMirrorMetrics();
      syncSpellMirrorScroll();
      syncPreviewWidth();
      bindPreviewResizeObserver();
      void renderPreview(props.modelValue);
    });
  },
);

watch(
  () => props.modelValue,
  () => {
    void nextTick(() => {
      syncSpellMirrorMetrics();
      syncSpellMirrorScroll();
      updateSpellPopoverPosition();
    });
  },
);

function onInput(value: string) {
  emit("updateModelValue", value);
}

function onCardClick(event: MouseEvent) {
  const linkUrl = resolvePreviewLinkUrl({
    target: event.target,
    clientX: event.clientX,
    clientY: event.clientY,
    scope: event.currentTarget instanceof Element ? event.currentTarget : null,
  });

  if (linkUrl) {
    event.preventDefault();
    event.stopPropagation();
    void openPreviewLinkUrl(linkUrl);
    return;
  }

  if (props.editing) {
    return;
  }

  const target = event.target instanceof Element ? event.target : null;
  if (shouldIgnoreBlockFocusClick(target)) {
    return;
  }

  emit("focus");
}
</script>

<template>
  <section class="typst-card" :class="{ editing }" @click="onCardClick">
    <template v-if="editing">
      <div class="editor-grid">
        <div class="pane spellcheck-pane">
          <div ref="textareaShellRef" class="spellcheck-editor-wrap">
            <v-textarea :model-value="modelValue" rows="3" max-rows="16" auto-grow hide-details density="compact"
             variant="solo-filled" class="code-input" @update:model-value="(value) => onInput(String(value ?? ''))"
             spellcheck="false" autocorrect="off" autocapitalize="off" autocomplete="off"
              placeholder="Input typst code here..."
              title="Shortcuts: Cmd/Ctrl+B bold, Cmd/Ctrl+I italic, Cmd/Ctrl+U underline, Cmd/Ctrl+Shift+1 note, +2 tip, +3 important, +4 warning, +5 caution, +6 todo"
              @blur="onEditorBlur" @keydown="onEditorKeydown" @focus="syncSpellMirrorMetrics" @update:focused="syncSpellMirrorMetrics" />
            <div v-if="spellHighlightLayerVisible" class="spellcheck-highlight-layer" :style="spellHighlightLayerStyle" aria-hidden="true">
              <div class="spellcheck-highlight-content" :style="spellMirrorStyle">
                <span
                  v-for="segment in spellHighlightSegments"
                  :ref="(el) => setSpellIssueSpanRef(segment.issue?.id, el)"
                  :key="segment.key"
                  class="spellcheck-highlight-segment"
                  :class="{
                    'is-issue': Boolean(segment.issue),
                    'is-active': segment.issue?.id === activeSpellIssueId,
                  }"
                  @pointerdown.stop.prevent="segment.issue && onSpellHighlightPointerDown(segment.issue.id)"
                >{{ segment.text }}</span>
              </div>
            </div>
          </div>
          <SpellcheckPanel
            ref="spellcheckPanelRef"
            title="Typst spellcheck"
            :issues="spellPanelIssues"
            :loading="spellLoading"
            :error="spellError"
            :visible="spellPanelVisible"
            :position="spellPopoverPosition"
            :placement="spellPopoverPlacement"
            empty-label="No spelling issues outside formulas."
            @pointerdown.capture="onSpellPanelPointerDown"
            @apply="onSpellPanelApply"
          />
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

.spellcheck-pane {
  position: relative;
  padding-bottom: 0;
}

.spellcheck-editor-wrap {
  position: relative;
}

.spellcheck-editor-wrap :deep(.v-field),
.spellcheck-editor-wrap :deep(.v-field__overlay),
.spellcheck-editor-wrap :deep(.v-field__field),
.spellcheck-editor-wrap :deep(.v-field__input) {
  background: transparent !important;
}

.spellcheck-editor-wrap :deep(textarea:not(.v-textarea__sizer)) {
  position: relative;
  z-index: 2;
  color: var(--fox-text-body) !important;
  caret-color: var(--fox-text-body);
  text-transform: none;
  font-variant-east-asian: normal;
  font-feature-settings: "fwid" 0, "hwid" 0, "pwid" 0, "palt" 0;
}

.spellcheck-editor-wrap :deep(textarea:not(.v-textarea__sizer)::selection) {
  background: color-mix(in srgb, var(--fox-primary) 35%, transparent);
  color: var(--fox-text-body);
}

.spellcheck-highlight-layer {
  position: absolute;
  inset: 0;
  z-index: 3;
  pointer-events: none;
  overflow: hidden;
}

.spellcheck-highlight-content {
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  min-height: 100%;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  color: transparent;
  -webkit-text-fill-color: transparent;
  text-transform: none;
  font-variant-east-asian: normal;
  font-feature-settings: "fwid" 0, "hwid" 0, "pwid" 0, "palt" 0;
}

.spellcheck-highlight-segment {
  pointer-events: none;
}

.spellcheck-highlight-segment.is-issue {
  pointer-events: auto;
  cursor: pointer;
   color: transparent;
   -webkit-text-fill-color: transparent;
  background: rgba(241, 14, 33, 0.13);
  text-decoration: underline #f10e21 solid 2px;
  text-underline-offset: 0.18em;
  border-radius: 0.22rem;
}

.spellcheck-highlight-segment.is-active {
  background: rgba(238, 66, 102, 0.24);
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

.preview-svg :deep(svg a .pseudo-link) {
  cursor: pointer;
}

.preview-svg :deep(svg foreignObject),
.preview-svg :deep(svg foreignObject *) {
  pointer-events: none;
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

  .spellcheck-pane {
    padding-bottom: 0;
  }
}
</style>
