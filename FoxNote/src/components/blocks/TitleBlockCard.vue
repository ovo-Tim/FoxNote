<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch, type ComputedRef, type Ref } from "vue";
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
let titleResizeObserver: ResizeObserver | null = null;
let summaryResizeObserver: ResizeObserver | null = null;
let boundTitleInput: HTMLInputElement | null = null;
let boundSummaryInput: HTMLTextAreaElement | null = null;

type TextInputControl = HTMLInputElement | HTMLTextAreaElement;

interface InlineSpellField {
  shellRef: Ref<HTMLElement | null>;
  inputRef: Ref<TextInputControl | null>;
  panelRef: Ref<InstanceType<typeof SpellcheckPanel> | null>;
  activeIssueId: Ref<string | null>;
  popoverPosition: Ref<{ left: number; top: number } | null>;
  popoverPlacement: Ref<"top" | "bottom">;
  scrollTop: Ref<number>;
  scrollLeft: Ref<number>;
  offsetTop: Ref<number>;
  offsetLeft: Ref<number>;
  width: Ref<number>;
  height: Ref<number>;
  paddingTop: Ref<number>;
  paddingRight: Ref<number>;
  paddingBottom: Ref<number>;
  paddingLeft: Ref<number>;
  fontSize: Ref<string>;
  lineHeight: Ref<string>;
  fontFamily: Ref<string>;
  fontWeight: Ref<string>;
  letterSpacing: Ref<string>;
  tabSize: Ref<string>;
  highlightSegments: ComputedRef<SpellcheckHighlightSegment[]>;
  panelIssues: ComputedRef<SpellcheckIssue[]>;
  panelVisible: ComputedRef<boolean>;
  layerVisible: ComputedRef<boolean>;
  layerStyle: ComputedRef<Record<string, string>>;
  mirrorStyle: ComputedRef<Record<string, string>>;
  setInputElement: (element: Element | null) => void;
  bindControlElement: () => void;
  cleanupBindings: () => void;
  syncMetrics: () => void;
  syncScroll: () => void;
  updatePopoverPosition: () => void;
  setIssueSpanRef: (issueId: string | undefined, element: unknown) => void;
  syncActiveIssueFromInput: () => void;
  closePopover: () => void;
  containsTarget: (target: Node | null) => boolean;
}

const {
  issues: titleSpellIssues,
  loading: titleSpellLoading,
  error: titleSpellError,
  applySuggestion: applyTitleSuggestion,
} = useSpellcheckField({
  source: computed(() => props.modelValue),
  mode: "plaintext",
  enabled: computed(() => props.editing),
  onApplied: (nextText) => {
    emit("updateModelValue", nextText);
  },
});

const {
  issues: summarySpellIssues,
  loading: summarySpellLoading,
  error: summarySpellError,
  applySuggestion: applySummarySuggestion,
} = useSpellcheckField({
  source: computed(() => localSummary.value),
  mode: "plaintext",
  enabled: computed(() => props.editing),
  onApplied: (nextText) => {
    localSummary.value = nextText;
    emit("updateSummary", nextText);
  },
});

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

function createInlineSpellField(config: {
  source: () => string;
  issues: typeof titleSpellIssues;
  error: typeof titleSpellError;
  selector: string;
  multiline: boolean;
}): InlineSpellField {
  const shellRef = ref<HTMLElement | null>(null);
  const inputRef = ref<TextInputControl | null>(null);
  const panelRef = ref<InstanceType<typeof SpellcheckPanel> | null>(null);
  const activeIssueId = ref<string | null>(null);
  const popoverPosition = ref<{ left: number; top: number } | null>(null);
  const popoverPlacement = ref<"top" | "bottom">("bottom");
  const scrollTop = ref(0);
  const scrollLeft = ref(0);
  const offsetTop = ref(0);
  const offsetLeft = ref(0);
  const width = ref(0);
  const height = ref(0);
  const paddingTop = ref(12);
  const paddingRight = ref(12);
  const paddingBottom = ref(12);
  const paddingLeft = ref(12);
  const fontSize = ref("0.95rem");
  const lineHeight = ref("1.45");
  const fontFamily = ref('"IBM Plex Sans", "Segoe UI", sans-serif');
  const fontWeight = ref("400");
  const letterSpacing = ref("normal");
  const tabSize = ref("4");
  const issueSpanMap = new Map<string, HTMLElement>();

  const activeIssue = computed<SpellcheckIssue | null>(() => {
    if (!activeIssueId.value) {
      return null;
    }
    return config.issues.value.find((issue) => issue.id === activeIssueId.value) ?? null;
  });

  const highlightSegments = computed<SpellcheckHighlightSegment[]>(() =>
    buildSpellcheckHighlightSegments(config.source(), config.issues.value),
  );

  const panelIssues = computed(() => (activeIssue.value ? [activeIssue.value] : []));
  const panelVisible = computed(
    () => props.editing && !!inputRef.value && (Boolean(activeIssue.value) || Boolean(config.error.value)),
  );
  const layerVisible = computed(() => props.editing && width.value > 0 && height.value > 0);
  const layerStyle = computed(() => ({
    top: `${offsetTop.value}px`,
    left: `${offsetLeft.value}px`,
    width: `${width.value}px`,
    height: `${height.value}px`,
  }));
  const mirrorStyle = computed(() => ({
    padding: `${paddingTop.value}px ${paddingRight.value}px ${paddingBottom.value}px ${paddingLeft.value}px`,
    fontSize: fontSize.value,
    lineHeight: lineHeight.value,
    fontFamily: fontFamily.value,
    fontWeight: fontWeight.value,
    letterSpacing: letterSpacing.value,
    tabSize: tabSize.value,
    transform: config.multiline
      ? `translate(${-scrollLeft.value}px, ${-scrollTop.value}px)`
      : `translateX(${-scrollLeft.value}px)`,
  }));

  watch(
    config.issues,
    (issues) => {
      if (!issues.length) {
        activeIssueId.value = null;
        popoverPosition.value = null;
        return;
      }

      if (activeIssueId.value && !issues.some((issue) => issue.id === activeIssueId.value)) {
        activeIssueId.value = null;
        popoverPosition.value = null;
      }

      void nextTick(() => {
        updatePopoverPosition();
      });
    },
    { immediate: true },
  );

  function setInputElement(element: Element | null) {
    inputRef.value = element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement ? element : null;
    syncMetrics();
  }

  function resetMetrics() {
    scrollTop.value = 0;
    scrollLeft.value = 0;
    offsetTop.value = 0;
    offsetLeft.value = 0;
    width.value = 0;
    height.value = 0;
  }

  function getInputElement() {
    const shell = shellRef.value;
    if (
      !inputRef.value ||
      !inputRef.value.isConnected ||
      (shell instanceof HTMLElement && !shell.contains(inputRef.value))
    ) {
      inputRef.value = null;
    }

    if (!inputRef.value && shell) {
      setInputElement(shell.querySelector(config.selector));
    }
    return inputRef.value;
  }

  function cleanupBindings() {
    if (config.multiline) {
      summaryResizeObserver?.disconnect();
      summaryResizeObserver = null;
      if (boundSummaryInput) {
        boundSummaryInput.removeEventListener("scroll", syncScroll);
      }
      boundSummaryInput = null;
      inputRef.value = null;
      issueSpanMap.clear();
      resetMetrics();
      return;
    }

    titleResizeObserver?.disconnect();
    titleResizeObserver = null;
    if (boundTitleInput) {
      boundTitleInput.removeEventListener("scroll", syncScroll);
    }
    boundTitleInput = null;
    inputRef.value = null;
    issueSpanMap.clear();
    resetMetrics();
  }

  function bindControlElement() {
    const input = getInputElement();
    if (!input) {
      cleanupBindings();
      return;
    }

    if (config.multiline) {
      if (boundSummaryInput === input) {
        return;
      }
    } else if (boundTitleInput === input) {
      return;
    }

    cleanupBindings();
    if (config.multiline && input instanceof HTMLTextAreaElement) {
      boundSummaryInput = input;
      boundSummaryInput.addEventListener("scroll", syncScroll, { passive: true });
      if (typeof ResizeObserver !== "undefined") {
        summaryResizeObserver = new ResizeObserver(() => {
          syncMetrics();
          syncScroll();
          updatePopoverPosition();
        });
        summaryResizeObserver.observe(boundSummaryInput);
      }
    } else if (!config.multiline && input instanceof HTMLInputElement) {
      boundTitleInput = input;
      boundTitleInput.addEventListener("scroll", syncScroll, { passive: true });
      if (typeof ResizeObserver !== "undefined") {
        titleResizeObserver = new ResizeObserver(() => {
          syncMetrics();
          syncScroll();
          updatePopoverPosition();
        });
        titleResizeObserver.observe(boundTitleInput);
      }
    }

    syncMetrics();
    syncScroll();
  }

  function syncMetrics() {
    const input = getInputElement();
    const shell = shellRef.value;
    if (!input || !shell || typeof window === "undefined") {
      resetMetrics();
      return;
    }

    const styles = window.getComputedStyle(input);
    const inputRect = input.getBoundingClientRect();
    const shellRect = shell.getBoundingClientRect();

    if (input.clientWidth <= 0 || input.clientHeight <= 0 || shellRect.width <= 0 || shellRect.height <= 0) {
      resetMetrics();
      return;
    }

    offsetTop.value = inputRect.top - shellRect.top;
    offsetLeft.value = inputRect.left - shellRect.left;
    width.value = input.clientWidth;
    height.value = input.clientHeight;
    paddingTop.value = Number.parseFloat(styles.paddingTop) || 12;
    paddingRight.value = Number.parseFloat(styles.paddingRight) || 12;
    paddingBottom.value = Number.parseFloat(styles.paddingBottom) || 12;
    paddingLeft.value = Number.parseFloat(styles.paddingLeft) || 12;
    fontSize.value = styles.fontSize;
    lineHeight.value = styles.lineHeight;
    fontFamily.value = styles.fontFamily;
    fontWeight.value = styles.fontWeight;
    letterSpacing.value = styles.letterSpacing;
    tabSize.value = styles.tabSize;
  }

  function syncScroll() {
    const input = getInputElement();
    if (!input) {
      return;
    }

    scrollLeft.value = input.scrollLeft;
    scrollTop.value = input instanceof HTMLTextAreaElement ? input.scrollTop : 0;
    updatePopoverPosition();
  }

  function updatePopoverPosition() {
    const issue = activeIssue.value;
    if (!issue || typeof window === "undefined") {
      popoverPosition.value = null;
      return;
    }

    const panelElement = panelRef.value?.$el;
    if (!(panelElement instanceof HTMLElement)) {
      return;
    }

    const anchorElement = issueSpanMap.get(issue.id);
    if (!anchorElement) {
      popoverPosition.value = null;
      return;
    }

    const rect = anchorElement.getBoundingClientRect();
    const viewport = { width: window.innerWidth, height: window.innerHeight };
    const panelRect = panelElement.getBoundingClientRect();
    const placement = computeSpellcheckPopoverPlacement(rect, viewport, {
      width: panelRect.width || 340,
      height: panelRect.height || 220,
    });

    popoverPlacement.value = placement.placement;
    popoverPosition.value = { left: placement.left, top: placement.top };
  }

  function findIssueForSelection(start: number, end: number): SpellcheckIssue | null {
    return (
      config.issues.value.find((issue) => {
        if (start === end) {
          return start >= issue.start && start <= issue.end;
        }

        return start < issue.end && end > issue.start;
      }) ?? null
    );
  }

  function setIssueSpanRef(issueId: string | undefined, element: unknown) {
    if (!issueId) {
      return;
    }

    if (!(element instanceof HTMLElement)) {
      issueSpanMap.delete(issueId);
      return;
    }

    issueSpanMap.set(issueId, element);
  }

  function syncActiveIssueFromInput() {
    const input = getInputElement();
    if (!input) {
      closePopover();
      return;
    }

    const start = input.selectionStart ?? 0;
    const end = input.selectionEnd ?? start;
    const issue = findIssueForSelection(start, end);
    if (!issue) {
      closePopover();
      return;
    }

    activeIssueId.value = issue.id;
    void nextTick(() => {
      updatePopoverPosition();
    });
  }

  function closePopover() {
    activeIssueId.value = null;
    popoverPosition.value = null;
  }

  function containsTarget(target: Node | null) {
    if (!activeIssue.value || !target) {
      return false;
    }

    const panelElement = panelRef.value?.$el;
    const issueElement = activeIssueId.value ? issueSpanMap.get(activeIssueId.value) : null;
    return (
      (panelElement instanceof HTMLElement && panelElement.contains(target)) ||
      Boolean(issueElement && issueElement.contains(target))
    );
  }

  return {
    shellRef,
    inputRef,
    panelRef,
    activeIssueId,
    popoverPosition,
    popoverPlacement,
    scrollTop,
    scrollLeft,
    offsetTop,
    offsetLeft,
    width,
    height,
    paddingTop,
    paddingRight,
    paddingBottom,
    paddingLeft,
    fontSize,
    lineHeight,
    fontFamily,
    fontWeight,
    letterSpacing,
    tabSize,
    highlightSegments,
    panelIssues,
    panelVisible,
    layerVisible,
    layerStyle,
    mirrorStyle,
    setInputElement,
    bindControlElement,
    cleanupBindings,
    syncMetrics,
    syncScroll,
    updatePopoverPosition,
    setIssueSpanRef,
    syncActiveIssueFromInput,
    closePopover,
    containsTarget,
  };
}

const titleSpellField = reactive(createInlineSpellField({
  source: () => props.modelValue,
  issues: titleSpellIssues,
  error: titleSpellError,
  selector: "input",
  multiline: false,
}));

const summarySpellField = reactive(createInlineSpellField({
  source: () => localSummary.value,
  issues: summarySpellIssues,
  error: summarySpellError,
  selector: "textarea",
  multiline: true,
}));

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

function onTitlePanelApply(issueId: string, suggestionId: string) {
  void applyTitleSuggestion(issueId, suggestionId).then(() => {
    titleSpellField.closePopover();
    void nextTick(() => {
      titleSpellField.updatePopoverPosition();
    });
  });
}

function onSummaryPanelApply(issueId: string, suggestionId: string) {
  void applySummarySuggestion(issueId, suggestionId).then(() => {
    summarySpellField.closePopover();
    void nextTick(() => {
      summarySpellField.updatePopoverPosition();
    });
  });
}

function onLevelInput(rawValue: string | number) {
  const parsed = Number(rawValue);
  if (!Number.isFinite(parsed)) {
    return;
  }
  const next = Math.min(6, Math.max(1, Math.round(parsed)));
  emit("updateLevel", next);
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

onMounted(() => {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return;
  }

  mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  syncDarkMode();
  titleSpellField.bindControlElement();
  summarySpellField.bindControlElement();
  mediaQuery.addEventListener("change", syncDarkMode);
  syncPreviewWidth();
  bindPreviewResizeObserver();
  void renderPreview();
  window.addEventListener("resize", updateInlineSpellPopovers);
  document.addEventListener("pointerdown", onDocumentPointerDown, true);
});

onBeforeUnmount(() => {
  mediaQuery?.removeEventListener("change", syncDarkMode);
  mediaQuery = null;
  previewResizeObserver?.disconnect();
  previewResizeObserver = null;
  titleSpellField.cleanupBindings();
  summarySpellField.cleanupBindings();
  window.removeEventListener("resize", updateInlineSpellPopovers);
  document.removeEventListener("pointerdown", onDocumentPointerDown, true);
});

function updateInlineSpellPopovers() {
  titleSpellField.updatePopoverPosition();
  summarySpellField.updatePopoverPosition();
}

function onDocumentPointerDown(event: PointerEvent) {
  const target = event.target instanceof Node ? event.target : null;
  if (!target) {
    return;
  }

  if (!titleSpellField.containsTarget(target)) {
    titleSpellField.closePopover();
  }
  if (!summarySpellField.containsTarget(target)) {
    summarySpellField.closePopover();
  }
}

watch(
  () => [props.modelValue, props.level, props.editing, props.folded, props.summary, prefersDark.value, previewPageWidth.value],
  () => {
    void renderPreview();
  },
);

watch(
  () => props.editing,
  (editing) => {
    if (!editing) {
      titleSpellField.cleanupBindings();
      summarySpellField.cleanupBindings();
      titleSpellField.closePopover();
      summarySpellField.closePopover();
      return;
    }

    void nextTick(() => {
      titleSpellField.bindControlElement();
      summarySpellField.bindControlElement();
      titleSpellField.syncMetrics();
      titleSpellField.syncScroll();
      summarySpellField.syncMetrics();
      summarySpellField.syncScroll();
      updateInlineSpellPopovers();
    });
  },
);

watch(
  () => props.modelValue,
  () => {
    void nextTick(() => {
      titleSpellField.syncMetrics();
      titleSpellField.syncScroll();
      titleSpellField.updatePopoverPosition();
    });
  },
);

watch(
  () => localSummary.value,
  () => {
    void nextTick(() => {
      summarySpellField.syncMetrics();
      summarySpellField.syncScroll();
      summarySpellField.updatePopoverPosition();
    });
  },
);
</script>

<template>
  <section class="title-card" :class="{ editing }" @click="onCardClick">
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
        <div ref="titleSpellField.shellRef" class="title-spellcheck-wrap">
          <v-text-field :model-value="modelValue" density="compact" variant="solo-filled" hide-details label="Title"
            class="title-input" @update:model-value="(value) => emit('updateModelValue', String(value ?? ''))"
            spellcheck="false" autocorrect="off" autocapitalize="off" autocomplete="off"
            @focus="titleSpellField.syncMetrics" @update:focused="titleSpellField.syncMetrics"
            @keyup="titleSpellField.syncActiveIssueFromInput" @select="titleSpellField.syncActiveIssueFromInput" />
          <div v-if="titleSpellField.layerVisible" class="spellcheck-highlight-layer spellcheck-highlight-layer--single" :style="titleSpellField.layerStyle" aria-hidden="true">
            <div class="spellcheck-highlight-content spellcheck-highlight-content--single" :style="titleSpellField.mirrorStyle">
              <span
                v-for="segment in titleSpellField.highlightSegments"
                :ref="(el) => titleSpellField.setIssueSpanRef(segment.issue?.id, el)"
                :key="segment.key"
                class="spellcheck-highlight-segment"
                :class="{
                  'is-issue': Boolean(segment.issue),
                  'is-active': segment.issue?.id === titleSpellField.activeIssueId,
                }"
              >{{ segment.text }}</span>
            </div>
          </div>
          <SpellcheckPanel
            ref="titleSpellField.panelRef"
            title="Title spellcheck"
            :issues="titleSpellField.panelIssues"
            :loading="titleSpellLoading"
            :error="titleSpellError"
            :visible="titleSpellField.panelVisible"
            :position="titleSpellField.popoverPosition"
            :placement="titleSpellField.popoverPlacement"
            @apply="onTitlePanelApply"
          />
        </div>

        <v-text-field :model-value="String(Math.min(6, Math.max(1, Number(level) || 1)))" density="compact"
          variant="solo-filled" hide-details type="number" min="1" max="6" label="Level" class="level-input"
          @update:model-value="onLevelInput" />
      </div>

      <div ref="summarySpellField.shellRef" class="summary-spellcheck-wrap">
        <v-textarea v-model="localSummary" rows="2" max-rows="5" auto-grow density="compact" variant="solo-filled"
          hide-details class="summary-input" label="Summary when folded"
          spellcheck="false" autocorrect="off" autocapitalize="off" autocomplete="off"
          placeholder="Optional summary shown when section is folded" @blur="onSummaryBlur"
          @focus="summarySpellField.syncMetrics" @update:focused="summarySpellField.syncMetrics"
          @keyup="summarySpellField.syncActiveIssueFromInput" @select="summarySpellField.syncActiveIssueFromInput" />
        <div v-if="summarySpellField.layerVisible" class="spellcheck-highlight-layer" :style="summarySpellField.layerStyle" aria-hidden="true">
          <div class="spellcheck-highlight-content" :style="summarySpellField.mirrorStyle">
            <span
              v-for="segment in summarySpellField.highlightSegments"
              :ref="(el) => summarySpellField.setIssueSpanRef(segment.issue?.id, el)"
              :key="segment.key"
              class="spellcheck-highlight-segment"
              :class="{
                'is-issue': Boolean(segment.issue),
                'is-active': segment.issue?.id === summarySpellField.activeIssueId,
              }"
            >{{ segment.text }}</span>
          </div>
        </div>
        <SpellcheckPanel
          ref="summarySpellField.panelRef"
          title="Summary spellcheck"
          :issues="summarySpellField.panelIssues"
          :loading="summarySpellLoading"
          :error="summarySpellError"
          :visible="summarySpellField.panelVisible"
          :position="summarySpellField.popoverPosition"
          :placement="summarySpellField.popoverPlacement"
          @apply="onSummaryPanelApply"
        />
      </div>
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

.title-preview :deep(svg a .pseudo-link) {
  cursor: pointer;
}

.title-preview :deep(svg foreignObject),
.title-preview :deep(svg foreignObject *) {
  pointer-events: none;
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
  align-items: start;
}

.title-spellcheck-wrap,
.summary-spellcheck-wrap {
  position: relative;
}

.title-spellcheck-wrap {
  min-width: 0;
}

.summary-spellcheck-wrap {
  margin-top: 0.38rem;
}

.title-spellcheck-wrap :deep(.v-field),
.title-spellcheck-wrap :deep(.v-field__overlay),
.title-spellcheck-wrap :deep(.v-field__field),
.title-spellcheck-wrap :deep(.v-field__input),
.summary-spellcheck-wrap :deep(.v-field),
.summary-spellcheck-wrap :deep(.v-field__overlay),
.summary-spellcheck-wrap :deep(.v-field__field),
.summary-spellcheck-wrap :deep(.v-field__input) {
  background: transparent !important;
}

.title-spellcheck-wrap :deep(input),
.summary-spellcheck-wrap :deep(textarea:not(.v-textarea__sizer)) {
  position: relative;
  z-index: 2;
  color: var(--fox-text-body) !important;
  -webkit-text-fill-color: var(--fox-text-body);
  caret-color: var(--fox-text-body);
  text-transform: none;
  font-variant-east-asian: normal;
  font-feature-settings: "fwid" 0, "hwid" 0, "pwid" 0, "palt" 0;
}

.title-spellcheck-wrap :deep(input::selection),
.summary-spellcheck-wrap :deep(textarea:not(.v-textarea__sizer)::selection) {
  background-color: Highlight;
  color: HighlightText;
  -webkit-text-fill-color: HighlightText;
}

.spellcheck-highlight-layer {
  position: absolute;
  inset: 0;
  z-index: 1;
  pointer-events: none;
  overflow: hidden;
}

.spellcheck-highlight-layer--single {
  display: flex;
  align-items: stretch;
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

.spellcheck-highlight-content--single {
  width: 100%;
  white-space: pre;
  overflow: hidden;
}

.spellcheck-highlight-segment {
  pointer-events: none;
}

.spellcheck-highlight-segment.is-issue {
  pointer-events: none;
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
