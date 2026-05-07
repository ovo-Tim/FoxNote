<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { searchNotes } from "../lib/noteApi";
import type { NoteSearchHit, TagEntry } from "../types/note";

const props = defineProps<{
  tags: TagEntry[];
}>();

const emit = defineEmits<{
  openNote: [noteId: string];
}>();

const searchViewQuery = ref("");
const searchViewTagPath = ref("");
const searchViewLoading = ref(false);
const searchViewResults = ref<NoteSearchHit[]>([]);
const searchRequestToken = ref(0);
let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

const tagToneColors = ["#ffba3d", "#ff7f3e", "#cbcbcb", "#43dfda", "#cf87ff"];

interface SearchTagItem {
  title: string;
  value: string;
  path: string;
  noteCount: number;
  color: string;
  isAll?: boolean;
}

const hasSearchQuery = computed(() => searchViewQuery.value.trim().length > 0);
const activeSearchTag = computed(() => searchViewTagPath.value.trim());
const hasSearchContext = computed(() => hasSearchQuery.value || activeSearchTag.value.length > 0);

const searchTagLabel = computed(() => {
  if (!activeSearchTag.value) {
    return "All tags in index";
  }
  const matched = props.tags.find((tag) => tag.path === activeSearchTag.value);
  return matched ? matched.path : activeSearchTag.value;
});

function tagToneColor(tagPath: string): string {
  let hash = 0;
  for (const char of tagPath) {
    hash = (hash << 5) - hash + char.charCodeAt(0);
    hash |= 0;
  }
  return tagToneColors[Math.abs(hash) % tagToneColors.length] ?? "#cbcbcb";
}

const searchTagItems = computed<SearchTagItem[]>(() => {
  const items: SearchTagItem[] = props.tags.map((tag) => ({
    title: `${tag.path} (${tag.noteCount})`,
    value: tag.path,
    path: tag.path,
    noteCount: tag.noteCount,
    color: tagToneColor(tag.path),
  }));

  return [{ title: "All tags", value: "", path: "All tags", noteCount: 0, color: "#687289", isAll: true }, ...items];
});

async function runSearchViewQuery() {
  const token = ++searchRequestToken.value;
  const query = searchViewQuery.value.trim();
  const tagPath = searchViewTagPath.value.trim();

  if (!query && !tagPath) {
    searchViewResults.value = [];
    searchViewLoading.value = false;
    return;
  }

  searchViewLoading.value = true;
  try {
    const results = await searchNotes(query, tagPath || undefined, 120);
    if (token === searchRequestToken.value) {
      searchViewResults.value = results;
    }
  } catch {
    if (token === searchRequestToken.value) {
      searchViewResults.value = [];
    }
  } finally {
    if (token === searchRequestToken.value) {
      searchViewLoading.value = false;
    }
  }
}

function scheduleSearchViewQuery(delayMs = 180) {
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = null;
  }

  searchDebounceTimer = setTimeout(() => {
    searchDebounceTimer = null;
    void runSearchViewQuery();
  }, delayMs);
}

function clearSearchViewContext() {
  searchViewQuery.value = "";
  searchViewTagPath.value = "";
  searchViewResults.value = [];
  searchViewLoading.value = false;
  searchRequestToken.value += 1;
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = null;
  }
}

watch(searchViewQuery, () => {
  scheduleSearchViewQuery();
});

watch(searchViewTagPath, () => {
  scheduleSearchViewQuery(0);
});

watch(
  () => props.tags,
  () => {
    if (hasSearchContext.value) {
      scheduleSearchViewQuery(0);
    }
  },
  { deep: true },
);

onBeforeUnmount(() => {
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = null;
  }
});
</script>

<template>
  <section class="search-view">
    <header class="search-header">
      <h2>Search Notes</h2>
      <p>Full-text search powered by Tantivy. Filter by tag branch when needed.</p>
    </header>
    <div class="search-controls">
      <v-text-field
        v-model="searchViewQuery"
        density="comfortable"
        variant="outlined"
        hide-details
        prepend-inner-icon="mdi-magnify"
        placeholder="Search title/content/folder..."
      />
      <v-select
        v-model="searchViewTagPath"
        :items="searchTagItems"
        item-title="title"
        item-value="value"
        density="comfortable"
        variant="outlined"
        hide-details
        label="Tag filter"
      >
        <template #item="{ props: itemProps, item }">
          <v-list-item v-bind="itemProps" :title="item.raw.title">
            <template #prepend>
              <span class="search-tag-dot" :style="{ backgroundColor: item.raw.color }" />
            </template>
          </v-list-item>
        </template>
        <template #selection="{ item }">
          <span class="search-tag-selection">
            <span class="search-tag-dot" :style="{ backgroundColor: item.raw.color }" />
            <span>{{ item.raw.isAll ? "All tags" : item.raw.path }}</span>
          </span>
        </template>
      </v-select>
      <v-btn
        variant="text"
        prepend-icon="mdi-close-circle-outline"
        :disabled="!hasSearchContext && !searchViewResults.length"
        @click="clearSearchViewContext"
      >
        Clear
      </v-btn>
    </div>

    <p v-if="hasSearchContext" class="search-context">
      Tag scope: <strong>{{ searchTagLabel }}</strong>
    </p>

    <div class="search-results" v-if="hasSearchContext">
      <div v-if="searchViewLoading" class="search-loading">Searching...</div>
      <button
        v-for="note in searchViewResults"
        :key="`search-${note.id}`"
        type="button"
        class="result-row"
        @click="emit('openNote', note.id)"
      >
        <span class="result-title">{{ note.title || "Untitled Note" }}</span>
        <span class="result-meta">{{ note.folder || 'root' }} · {{ note.date }} · score {{ note.score.toFixed(2) }}</span>
        <span v-if="note.snippet" class="result-snippet">{{ note.snippet }}</span>
        <span v-if="note.tags.length" class="result-tags">{{ note.tags.join(" · ") }}</span>
      </button>
      <p v-if="!searchViewLoading && searchViewResults.length === 0" class="search-empty">No matching notes.</p>
    </div>
    <p v-else class="search-empty">Type a query or choose a tag filter to start.</p>
  </section>
</template>

<style scoped>
.search-view {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0.85rem 0.95rem;
}

.search-header h2 {
  margin: 0;
  color: var(--fox-text-strong);
}

.search-header p {
  margin: 0.2rem 0 0.7rem;
  color: var(--fox-text-muted);
}

.search-controls {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(180px, 1fr) auto;
  gap: 0.55rem;
  align-items: center;
}

.search-context {
  margin: 0.55rem 0 0;
  color: var(--fox-text-muted);
  font-size: 0.86rem;
}

.search-tag-selection {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
}

.search-tag-dot {
  width: 0.62rem;
  height: 0.62rem;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, #0b0f18 36%, transparent 64%);
  box-shadow: 0 0 0 1px color-mix(in srgb, white 12%, transparent 88%);
  flex: 0 0 auto;
}

.search-results {
  margin-top: 0.65rem;
  display: flex;
  flex-direction: column;
  gap: 0.28rem;
}

.search-loading {
  color: var(--fox-text-muted);
  font-size: 0.86rem;
  margin-bottom: 0.2rem;
}

.result-row {
  width: 100%;
  border: 1px solid var(--fox-border);
  border-radius: 9px;
  background: color-mix(in srgb, var(--fox-surface) 90%, black 10%);
  color: var(--fox-text-body);
  text-align: left;
  padding: 0.5rem 0.65rem;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 0.12rem;
}

.result-row:hover {
  background: color-mix(in srgb, var(--fox-chip) 78%, transparent 22%);
}

.result-title {
  color: var(--fox-text-strong);
  font-weight: 600;
}

.result-meta {
  color: var(--fox-text-muted);
  font-size: 0.82rem;
}

.result-snippet {
  color: var(--fox-text-body);
  font-size: 0.84rem;
  line-height: 1.35;
}

.result-tags {
  color: var(--fox-text-muted);
  font-size: 0.78rem;
}

.search-empty {
  color: var(--fox-text-muted);
  margin: 0.35rem 0 0;
}

@media (max-width: 760px) {
  .search-view {
    padding: 0.7rem 0.65rem;
  }

  .search-controls {
    grid-template-columns: 1fr;
  }
}
</style>
