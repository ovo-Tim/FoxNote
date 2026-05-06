<script setup lang="ts">
import { computed } from "vue";
import { blockPreviewText } from "../../editor/utils";
import type { NoteBlock } from "../../types/note";

const props = defineProps<{
  block: NoteBlock;
}>();

const preview = computed(() => blockPreviewText(props.block));
</script>

<template>
  <section class="unsupported-card">
    <p class="type-label">{{ block.type }}</p>
    <pre class="preview">{{ preview }}</pre>
  </section>
</template>

<style scoped>
.unsupported-card {
  border: 1px dashed color-mix(in srgb, var(--fox-border) 86%, transparent 14%);
  border-radius: 12px;
  background: color-mix(in srgb, var(--fox-surface) 90%, black 10%);
  padding: 0.6rem;
}

.type-label {
  margin: 0 0 0.35rem;
  color: var(--fox-text-muted);
  font-size: 0.74rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.preview {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--fox-text-body);
  font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
  font-size: 0.95rem;
}
</style>
