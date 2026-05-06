<script setup lang="ts">
const props = defineProps<{
  title: string;
  subtitle?: string;
  activeTagLabel?: string;
  searchQuery?: string;
}>();

const emit = defineEmits<{
  refresh: [];
  clearTagFilter: [];
  updateSearchQuery: [value: string];
}>();

function onRefresh() {
  emit("refresh");
}

function onSearch(value: string) {
  emit("updateSearchQuery", value);
}

function onClearTag() {
  emit("clearTagFilter");
}
</script>

<template>
  <header class="app-header">
    <div class="window-row">
      <div class="traffic-lights" aria-hidden="true">
        <span class="dot red" />
        <span class="dot amber" />
        <span class="dot green" />
      </div>

      <div class="title-wrap">
        <p class="title-text">{{ title }}</p>
        <p v-if="subtitle" class="subtitle-text">{{ subtitle }}</p>
      </div>

      <div class="header-actions">
        <v-btn
          v-if="activeTagLabel && activeTagLabel !== 'All notes'"
          variant="tonal"
          size="small"
          prepend-icon="mdi-tag-off-outline"
          @click="onClearTag"
        >
          {{ activeTagLabel }}
        </v-btn>
        <v-btn variant="text" icon="mdi-refresh" @click="onRefresh" />
      </div>
    </div>

    <div class="toolbar-row">
      <v-text-field
        :model-value="props.searchQuery ?? ''"
        density="compact"
        variant="solo-filled"
        hide-details
        rounded="lg"
        prepend-inner-icon="mdi-magnify"
        placeholder="Quick search"
        class="search-input"
        @update:model-value="(value) => onSearch(String(value ?? ''))"
      />

    </div>
  </header>
</template>

<style scoped>
.app-header {
  border: 1px solid var(--fox-border);
  border-radius: 12px;
  background: var(--fox-chrome);
  padding: 0.45rem 0.55rem 0.55rem;
}

.window-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.85rem;
  padding: 0.1rem 0.2rem 0.45rem;
}

.traffic-lights {
  display: flex;
  gap: 0.4rem;
}

.dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
}

.red {
  background: #f56c6c;
}

.amber {
  background: #f4c351;
}

.green {
  background: #5ec17c;
}

.title-wrap {
  flex: 1;
  min-width: 0;
}

.title-text {
  margin: 0;
  font-size: 1rem;
  color: var(--fox-text-strong);
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.subtitle-text {
  margin: 0.1rem 0 0;
  font-size: 0.8rem;
  color: var(--fox-text-muted);
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 0.45rem;
}

.toolbar-row {
  display: grid;
  grid-template-columns: minmax(240px, 1fr);
  gap: 0.5rem;
}

.search-input {
  min-width: 0;
}

@media (max-width: 980px) {
  .toolbar-row {
    grid-template-columns: 1fr;
  }
}
</style>
