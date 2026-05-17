<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { SpellcheckIssue } from "../editor/spellcheck";

const LINT_KIND_COLORS: Record<string, string> = {
  Spelling: "#EE4266",
  Grammar: "#9B59B6",
  Punctuation: "#D4850F",
  Style: "#FFD23F",
  Usage: "#1E90FF",
  Readability: "#2E8B57",
};

const props = defineProps<{
  title: string;
  issues: SpellcheckIssue[];
  loading: boolean;
  error: string;
  emptyLabel?: string;
  visible?: boolean;
  placement?: "top" | "bottom";
  position?: {
    left: number;
    top: number;
  } | null;
}>();

const emit = defineEmits<{
  apply: [issueId: string, suggestionId: string];
}>();

const dismissedIssueIds = ref<string[]>([]);
const activeIndex = ref(0);

const visibleIssues = computed(() => props.issues.filter((issue) => !dismissedIssueIds.value.includes(issue.id)));

const activeIssue = computed(() => visibleIssues.value[activeIndex.value] ?? null);
const activeIssueNumber = computed(() => (activeIssue.value ? activeIndex.value + 1 : 0));
const shouldRender = computed(
  () => (props.visible ?? true) && (props.loading || Boolean(props.error) || Boolean(activeIssue.value)),
);
const activeAccentColor = computed(() => {
  if (!activeIssue.value) {
    return "#EE4266";
  }
  return LINT_KIND_COLORS[activeIssue.value.kind] ?? "#EE4266";
});

watch(
  () => props.issues.map((issue) => issue.id),
  (nextIds) => {
    dismissedIssueIds.value = dismissedIssueIds.value.filter((id) => nextIds.includes(id));
    if (activeIndex.value >= visibleIssues.value.length) {
      activeIndex.value = 0;
    }
  },
  { immediate: true },
);

watch(
  () => visibleIssues.value.length,
  (count) => {
    if (count === 0 || activeIndex.value < count) {
      return;
    }
    activeIndex.value = 0;
  },
);

const panelTitle = computed(() => activeIssue.value?.kind || props.title || "Spelling");
const panelStyle = computed(() => ({
  "--spellcheck-accent": activeAccentColor.value,
  "--spellcheck-accent-soft": `${activeAccentColor.value}22`,
  left: props.position ? `${props.position.left}px` : undefined,
  top: props.position ? `${props.position.top}px` : undefined,
}));

function dismissCurrentIssue() {
  if (!activeIssue.value) {
    return;
  }

  dismissedIssueIds.value = [...dismissedIssueIds.value, activeIssue.value.id];
  if (activeIndex.value >= visibleIssues.value.length - 1) {
    activeIndex.value = 0;
  }
}

function focusNextIssue() {
  if (visibleIssues.value.length <= 1) {
    return;
  }
  activeIndex.value = (activeIndex.value + 1) % visibleIssues.value.length;
}
</script>

<template>
  <section
    v-if="shouldRender"
    class="spellcheck-panel"
    :class="{
      'is-floating': Boolean(position),
      'placement-top': placement === 'top',
      'placement-bottom': placement !== 'top',
    }"
    :style="panelStyle"
    aria-live="polite"
  >
    <div class="spellcheck-accent" />
    <div class="spellcheck-header">
      <div class="spellcheck-title-wrap">
        <span class="spellcheck-title">{{ panelTitle }}</span>
        <span v-if="!loading && !error && activeIssue" class="spellcheck-kind-chip">{{ activeIssue.kind }}</span>
      </div>
      <div class="spellcheck-header-actions">
        <span v-if="!loading && !error && visibleIssues.length" class="spellcheck-count">
          {{ activeIssueNumber }}/{{ visibleIssues.length }}
        </span>
        <button
          v-if="!loading && !error && visibleIssues.length > 1"
          type="button"
          class="spellcheck-icon-btn"
          title="Next issue"
          @pointerdown.prevent.stop
          @click="focusNextIssue"
        >
          ›
        </button>
        <button
          v-if="!loading && !error && activeIssue"
          type="button"
          class="spellcheck-icon-btn"
          title="Dismiss"
          @pointerdown.prevent.stop
          @click="dismissCurrentIssue"
        >
          ×
        </button>
      </div>
    </div>

    <p v-if="loading" class="spellcheck-copy">Checking writing…</p>
    <p v-else-if="error" class="spellcheck-error">{{ error }}</p>
    <template v-else-if="activeIssue">
      <p class="spellcheck-copy">
        Did you mean to spell
        <span v-if="activeIssue.problemText" class="spellcheck-problem">{{ activeIssue.problemText }}</span>
        <span v-else>this</span>
        this way?
      </p>
      <p class="spellcheck-message">{{ activeIssue.message }}</p>

      <div v-if="activeIssue.suggestions.length" class="spellcheck-actions">
        <button
          v-for="suggestion in activeIssue.suggestions"
          :key="suggestion.id"
          type="button"
          class="spellcheck-suggestion"
          @pointerdown.prevent.stop
          @click="emit('apply', activeIssue.id, suggestion.id)"
        >
          {{ suggestion.label }}
        </button>
        <button type="button" class="spellcheck-dismiss" @pointerdown.prevent.stop @click="dismissCurrentIssue">Dismiss</button>
      </div>
      <p v-else class="spellcheck-empty">{{ emptyLabel ?? "No suggestions available for this issue." }}</p>
    </template>
  </section>
</template>

<style scoped>
.spellcheck-panel {
  position: relative;
  width: min(100%, 21.25rem);
  max-width: min(calc(100vw - 2rem), 21.25rem);
  padding: 0.72rem 0.82rem 0.82rem;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 14px;
  background: #11151d;
  box-shadow:
    0 24px 48px rgba(0, 0, 0, 0.46),
    0 0 0 1px rgba(255, 255, 255, 0.04) inset;
  color: #eef2f8;
  animation: spellcheck-fade-in 140ms ease-out;
}

.spellcheck-panel.is-floating {
  position: fixed;
  z-index: 32;
  margin: 0;
}

.spellcheck-panel.is-floating.placement-top {
  transform-origin: bottom left;
}

.spellcheck-panel.is-floating.placement-bottom {
  transform-origin: top left;
}

@media (max-width: 980px) {
  .spellcheck-panel,
  .spellcheck-panel.is-floating {
    position: relative;
    inset: auto;
    margin-top: 0.65rem;
    width: min(100%, 100%);
    max-width: 100%;
  }
}

.spellcheck-accent {
  position: absolute;
  inset: 0 0 auto;
  height: 3px;
  border-radius: 14px 14px 0 0;
  background: linear-gradient(90deg, var(--spellcheck-accent) 0%, color-mix(in srgb, var(--spellcheck-accent) 82%, white) 100%);
}

.spellcheck-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.spellcheck-title-wrap {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  min-width: 0;
}

.spellcheck-header-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}

.spellcheck-title {
  font-size: 0.98rem;
  font-weight: 700;
  color: #f5f7fb;
}

.spellcheck-kind-chip {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.42rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--spellcheck-accent) 16%, #11151d);
  color: color-mix(in srgb, var(--spellcheck-accent) 78%, white);
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.01em;
  border: 1px solid color-mix(in srgb, var(--spellcheck-accent) 42%, transparent);
}

.spellcheck-count {
  font-size: 0.74rem;
  color: rgba(231, 238, 247, 0.72);
}

.spellcheck-icon-btn {
  border: 0;
  background: transparent;
  color: rgba(231, 238, 247, 0.76);
  width: 1.75rem;
  height: 1.75rem;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 1rem;
  cursor: pointer;
  transition: background-color 120ms ease, color 120ms ease;
}

.spellcheck-icon-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #ffffff;
}

.spellcheck-error {
  margin: 0.65rem 0 0;
  color: #ffb3bf;
  font-size: 0.9rem;
}

.spellcheck-copy {
  margin: 0.72rem 0 0;
  font-size: 0.98rem;
  line-height: 1.45;
  color: rgba(244, 247, 252, 0.94);
}

.spellcheck-problem {
  display: inline-block;
  margin: 0 0.18rem;
  padding: 0 0.18rem;
  border-radius: 0.28rem;
  background: color-mix(in srgb, var(--spellcheck-accent) 20%, #11151d);
  color: color-mix(in srgb, var(--spellcheck-accent) 72%, white);
  text-decoration: underline;
  text-decoration-color: var(--spellcheck-accent);
  text-decoration-thickness: 2px;
  text-underline-offset: 0.08em;
}

.spellcheck-message {
  margin: 0.45rem 0 0;
  font-size: 0.84rem;
  line-height: 1.45;
  color: rgba(219, 227, 238, 0.72);
}

.spellcheck-actions {
  margin-top: 0.82rem;
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.spellcheck-suggestion,
.spellcheck-dismiss {
  padding: 0.44rem 0.72rem;
  border-radius: 999px;
  border: 0;
  font-size: 0.9rem;
  font-weight: 700;
  cursor: pointer;
  transition: transform 120ms ease, filter 120ms ease, background-color 120ms ease;
}

.spellcheck-suggestion {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--spellcheck-accent) 88%, white) 0%,
    var(--spellcheck-accent) 100%
  );
  color: white;
}

.spellcheck-dismiss {
  background: #f4f6fa;
  color: #121722;
}

.spellcheck-suggestion:hover,
.spellcheck-dismiss:hover {
  transform: translateY(-1px);
  filter: brightness(1.04);
}

.spellcheck-empty {
  margin: 0.78rem 0 0;
  font-size: 0.84rem;
  color: rgba(219, 227, 238, 0.72);
}

@keyframes spellcheck-fade-in {
  from {
    opacity: 0;
    transform: translateY(4px);
  }

  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
