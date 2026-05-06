<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import TypstBlockCard from "./blocks/TypstBlockCard.vue";
import UnsupportedBlockCard from "./blocks/UnsupportedBlockCard.vue";
import { createDefaultBlockForType, listBlockPlugins } from "../editor/plugins/registry";
import { cloneBlocks } from "../editor/utils";
import type { NoteBlock, NoteDocument, NoteRecord } from "../types/note";

const props = defineProps<{
  note: NoteRecord | null;
  tagOptions?: string[];
}>();

const emit = defineEmits<{
  change: [document: NoteDocument];
}>();

const form = reactive<NoteDocument>({
  title: "",
  date: "",
  type: "notes",
  tags: [],
  content: [],
});

const tagMenuOpen = ref(false);
const tagQuery = ref("");
const slashMenuOpen = ref(false);
const slashMenuAnchorIndex = ref<number | null>(null);
const slashQuery = ref("");
const editingBlockIndex = ref<number | null>(null);
const isHydratingForm = ref(false);
const titleEditing = ref(false);
const titleInputRef = ref<HTMLInputElement | null>(null);

const tagToneClasses = ["tone-amber", "tone-orange", "tone-gray", "tone-cyan", "tone-magenta"];
const blockPlugins = listBlockPlugins();

const availableTagOptions = computed(() => {
  const base = props.tagOptions ?? [];
  const current = form.tags;
  const merged = new Set<string>([...base, ...current]);
  return [...merged].sort((left, right) => left.localeCompare(right));
});

const noteTypeLabel = computed(() => {
  if (form.type === "notes") {
    return "Page";
  }
  return form.type || "Page";
});

const normalizedTagQuery = computed(() => {
  return normalizeTagCandidate(tagQuery.value);
});

const filteredTagOptions = computed(() => {
  const query = normalizedTagQuery.value.toLowerCase();
  if (!query) {
    return availableTagOptions.value;
  }

  return availableTagOptions.value.filter((option) => option.toLowerCase().includes(query));
});

const showCreateTagOption = computed(() => {
  const candidate = normalizedTagQuery.value;
  return candidate.length > 0 && !availableTagOptions.value.includes(candidate);
});

const blockPaletteOptions = computed(() => {
  const query = slashQuery.value.trim().toLowerCase();
  if (!query) {
    return blockPlugins;
  }

  return blockPlugins.filter((plugin) => {
    const indexText = `${plugin.label} ${plugin.type} ${plugin.description}`.toLowerCase();
    return indexText.includes(query);
  });
});

const canShowSlashMenu = computed(() => {
  return slashMenuOpen.value && slashMenuAnchorIndex.value !== null;
});

function normalizeTagInput(values: unknown) {
  if (!Array.isArray(values)) {
    form.tags = [];
    return;
  }

  form.tags = values
    .filter((value): value is string => typeof value === "string")
    .map((value) => value.trim())
    .filter(Boolean)
    .filter((value, index, array) => array.indexOf(value) === index);
}

function normalizeTagCandidate(raw: string): string {
  return raw.trim().replace(/\s+/g, "-").replace(/\/{2,}/g, "/").replace(/^\/+|\/+$/g, "");
}

function ensureAtLeastOneBlock() {
  if (form.content.length > 0) {
    return;
  }

  const defaultBlock = createDefaultBlockForType("typst") ?? {
    type: "typst",
    content: "",
  };
  form.content = [defaultBlock];
}

function isTagSelected(tag: string): boolean {
  return form.tags.includes(tag);
}

function toggleTag(tag: string) {
  if (isTagSelected(tag)) {
    normalizeTagInput(form.tags.filter((item) => item !== tag));
    return;
  }

  normalizeTagInput([...form.tags, tag]);
}

function createTagFromQuery() {
  const candidate = normalizedTagQuery.value;
  if (!candidate) {
    return;
  }

  if (!isTagSelected(candidate)) {
    normalizeTagInput([...form.tags, candidate]);
  }
  tagQuery.value = "";
}

function tagToneClass(tag: string): string {
  let hash = 0;
  for (const char of tag) {
    hash = (hash << 5) - hash + char.charCodeAt(0);
    hash |= 0;
  }
  return tagToneClasses[Math.abs(hash) % tagToneClasses.length] ?? "tone-gray";
}

function updateBlock(index: number, nextBlock: NoteBlock) {
  const blocks = cloneBlocks(form.content);
  if (!blocks[index]) {
    return;
  }
  blocks[index] = nextBlock;
  form.content = blocks;
}

function updateTypstBlock(index: number, value: string) {
  const block = form.content[index];
  if (!block || block.type !== "typst") {
    return;
  }

  updateBlock(index, {
    ...block,
    content: value,
  });
}

function setEditingBlock(index: number | null) {
  editingBlockIndex.value = index;
}

function openSlashMenu(index: number) {
  slashMenuAnchorIndex.value = index;
  slashMenuOpen.value = true;
  slashQuery.value = "";
}

function closeSlashMenu() {
  slashMenuOpen.value = false;
  slashMenuAnchorIndex.value = null;
  slashQuery.value = "";
}

function insertBlockAfter(index: number, blockType: string) {
  const newBlock = createDefaultBlockForType(blockType);
  if (!newBlock) {
    return;
  }

  const blocks = cloneBlocks(form.content);
  blocks.splice(index + 1, 0, newBlock);
  form.content = blocks;
  setEditingBlock(index + 1);
  closeSlashMenu();
}

function onBlockInput(index: number, nextValue: string) {
  updateTypstBlock(index, nextValue);

  if (nextValue.trim() === "/") {
    updateTypstBlock(index, "");
    openSlashMenu(index);
  }
}

function removeBlock(index: number) {
  const blocks = cloneBlocks(form.content);
  if (!blocks[index]) {
    return;
  }

  blocks.splice(index, 1);
  form.content = blocks;
  ensureAtLeastOneBlock();
  setEditingBlock(null);
  closeSlashMenu();
}

function startTitleEdit() {
  titleEditing.value = true;
  void nextTick(() => {
    titleInputRef.value?.focus();
    titleInputRef.value?.select();
  });
}

function finishTitleEdit() {
  titleEditing.value = false;
  if (!form.title.trim()) {
    form.title = "Untitled Note";
  }
}

function exitBlockEditMode() {
  setEditingBlock(null);
}

watch(
  () => props.note,
  (note) => {
    isHydratingForm.value = true;

    if (!note) {
      form.title = "";
      form.date = "";
      form.type = "notes";
      form.tags = [];
      form.content = [];
      setEditingBlock(null);
      closeSlashMenu();
      titleEditing.value = false;
      isHydratingForm.value = false;
      return;
    }

    form.title = note.document.title;
    form.date = note.document.date;
    form.type = note.document.type;
    form.tags = [...note.document.tags];
    form.content = cloneBlocks(note.document.content);
    ensureAtLeastOneBlock();
    setEditingBlock(null);
    closeSlashMenu();
    titleEditing.value = false;
    isHydratingForm.value = false;
  },
  { immediate: true },
);

function emitChange() {
  if (!props.note || isHydratingForm.value) {
    return;
  }

  ensureAtLeastOneBlock();
  emit("change", {
    title: form.title,
    date: form.date,
    type: form.type,
    tags: [...form.tags],
    content: cloneBlocks(form.content),
  });
}

watch(form, emitChange, { deep: true });
</script>

<template>
  <section class="editor-pane" v-if="note" @click.self="exitBlockEditMode">
    <header class="note-top">
      <div class="meta-wrap">
        <button
          v-if="!titleEditing"
          type="button"
          class="title-display"
          @click="startTitleEdit"
        >
          {{ form.title || "Untitled" }}
        </button>
        <input
          v-else
          ref="titleInputRef"
          v-model="form.title"
          class="title-input"
          type="text"
          @blur="finishTitleEdit"
          @keydown.enter.prevent="finishTitleEdit"
          @keydown.esc.prevent="finishTitleEdit"
        />
        <p class="meta-line">
          <span>{{ noteTypeLabel }}</span>
          <span class="dot">•</span>
          <v-menu v-model="tagMenuOpen" location="bottom start" :close-on-content-click="false">
            <template #activator="{ props: menuProps }">
              <button type="button" class="tag-area-trigger" v-bind="menuProps">
                <template v-if="form.tags.length > 0">
                  <span v-for="tag in form.tags" :key="`meta-${tag}`" class="meta-tag-pill" :class="tagToneClass(tag)">
                    {{ tag }}
                  </span>
                </template>
                <span v-else class="empty-tag-label">No tags</span>
              </button>
            </template>

            <div class="tag-menu">
              <p class="tag-menu-label">Tag</p>
              <v-text-field v-model="tagQuery" density="compact" variant="solo-filled" hide-details rounded="pill"
                placeholder="Filter or create options..." class="tag-filter-input"
                @keydown.enter.prevent="createTagFromQuery" />

              <div class="tag-options">
                <button v-for="tag in filteredTagOptions" :key="`tag-option-${tag}`" type="button"
                  class="tag-option-row" @click="toggleTag(tag)">
                  <span class="drag-handle" aria-hidden="true">⋮⋮</span>
                  <span class="tag-pill" :class="tagToneClass(tag)">{{ tag }}</span>
                  <v-icon v-if="isTagSelected(tag)" icon="mdi-check" size="16" />
                </button>

                <button v-if="showCreateTagOption" type="button" class="tag-option-row create-row"
                  @click="createTagFromQuery">
                  <v-icon icon="mdi-plus-circle-outline" size="16" />
                  <span>Create "{{ normalizedTagQuery }}"</span>
                </button>
              </div>
            </div>
          </v-menu>
        </p>
      </div>
    </header>

    <section class="block-stack" @click.self="exitBlockEditMode">
      <article v-for="(block, index) in form.content" :key="`${note.id}-block-${index}-${block.type}`"
        class="block-shell">
        <div class="block-hover-actions">
          <v-btn icon="mdi-plus-circle-outline" size="x-small" variant="tonal" class="block-action-btn"
            @click="openSlashMenu(index)" />
          <v-btn v-if="form.content.length > 1" icon="mdi-trash-can-outline" size="x-small" variant="text"
            class="block-action-btn" @click="removeBlock(index)" />
        </div>

        <TypstBlockCard v-if="block.type === 'typst'"
          :model-value="typeof block.content === 'string' ? block.content : ''" :editing="editingBlockIndex === index"
          @focus="setEditingBlock(index)" @blur="setEditingBlock(null)"
          @update-model-value="(value) => onBlockInput(index, value)" />

        <UnsupportedBlockCard v-else :block="block" />

        <v-menu :model-value="canShowSlashMenu && slashMenuAnchorIndex === index" location="bottom start"
          :close-on-content-click="false" @update:model-value="(value) => !value && closeSlashMenu()">
          <template #activator="{ props: menuProps }">
            <span class="menu-anchor" v-bind="menuProps" />
          </template>

          <div class="slash-menu">
            <p class="slash-menu-title">Insert block</p>
            <v-text-field v-model="slashQuery" density="compact" variant="solo-filled" hide-details rounded="pill"
              placeholder="Type / to search block type" class="slash-search" />
            <div class="slash-options">
              <button v-for="plugin in blockPaletteOptions" :key="plugin.type" type="button" class="slash-option"
                @click="insertBlockAfter(index, plugin.type)">
                <span class="slash-option-label">/{{ plugin.type }}</span>
                <span class="slash-option-desc">{{ plugin.description }}</span>
              </button>
            </div>
          </div>
        </v-menu>
      </article>
    </section>

  </section>

  <section class="editor-pane empty" v-else>
    <h2>Editor</h2>
    <p>Choose a note from the tree, or create a new one.</p>
  </section>
</template>

<style scoped>
.editor-pane {
  border: 0;
  border-radius: 0;
  background: transparent;
  padding: 0.4rem 0.9rem 0.6rem;
  min-height: 100%;
}

.note-top {
  display: flex;
  align-items: flex-end;
  gap: 0.45rem;
}

.meta-wrap {
  flex: 1;
  min-width: 0;
}

.title-display {
  border: 0;
  background: transparent;
  padding: 0;
  margin: 0 0 0.15rem;
  font-family: inherit;
  font-size: 2.7rem;
  font-weight: 700;
  color: var(--fox-text-strong);
  line-height: 1.04;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  cursor: text;
}

.title-input {
  margin: 0 0 0.15rem;
  width: min(100%, 760px);
  border: 0;
  border-radius: 6px;
  background: color-mix(in srgb, var(--fox-chip) 70%, transparent 30%);
  padding: 0.12rem 0.3rem;
  font-family: inherit;
  font-size: 2.7rem;
  font-weight: 700;
  color: var(--fox-text-strong);
  line-height: 1.04;
}

.title-input:focus {
  outline: 1px solid color-mix(in srgb, var(--fox-primary) 45%, transparent 55%);
}

.meta-line {
  margin: 0;
  color: var(--fox-text-muted);
  font-size: 1.02rem;
  display: flex;
  align-items: center;
  gap: 0.4rem;
  flex-wrap: wrap;
}

.dot {
  opacity: 0.65;
}

.meta-tag-pill {
  display: inline-flex;
  align-items: center;
  border-radius: 6px;
  padding: 0.08rem 0.4rem;
  font-size: 0.88rem;
  line-height: 1.2;
}

.tag-area-trigger {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  flex-wrap: wrap;
  border: 0;
  background: transparent;
  padding: 0;
  margin: 0;
  cursor: pointer;
}

.empty-tag-label {
  color: var(--fox-text-muted);
  font-size: 0.9rem;
}

.tag-area-trigger:hover .meta-tag-pill,
.tag-area-trigger:hover .empty-tag-label {
  filter: brightness(1.08);
}

.tag-menu {
  width: min(345px, 72vw);
  border: 1px solid var(--fox-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--fox-surface) 90%, black 10%);
  padding: 0.65rem;
  box-shadow: 0 14px 30px rgba(0, 0, 0, 0.35);
}

.tag-menu-label {
  margin: 0 0 0.45rem;
  font-size: 1rem;
  color: var(--fox-text-strong);
  font-weight: 600;
}

.tag-filter-input {
  margin-bottom: 0.55rem;
}

.tag-options {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  max-height: 224px;
  overflow: auto;
}

.tag-option-row {
  width: 100%;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--fox-text-body);
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 0.45rem;
  padding: 0.22rem 0.28rem;
  text-align: left;
  cursor: pointer;
}

.tag-option-row:hover {
  background: var(--fox-chip);
}

.create-row {
  grid-template-columns: auto 1fr;
}

.drag-handle {
  color: var(--fox-text-muted);
  font-size: 0.82rem;
  letter-spacing: -0.04em;
}

.tag-pill {
  display: inline-flex;
  align-items: center;
  border-radius: 6px;
  padding: 0.1rem 0.42rem;
  width: fit-content;
  font-size: 0.9rem;
}

.tone-amber {
  background: rgba(255, 176, 0, 0.28);
  color: #ffba3d;
}

.tone-orange {
  background: rgba(255, 94, 0, 0.26);
  color: #ff7f3e;
}

.tone-gray {
  background: rgba(190, 190, 190, 0.2);
  color: #cbcbcb;
}

.tone-cyan {
  background: rgba(0, 199, 192, 0.25);
  color: #43dfda;
}

.tone-magenta {
  background: rgba(166, 52, 255, 0.24);
  color: #cf87ff;
}

.block-stack {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.block-shell {
  position: relative;
  margin: 0;
}

.block-shell + .block-shell {
  margin-top: 0;
}

.block-hover-actions {
  position: absolute;
  left: -2.15rem;
  top: 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  opacity: 0;
  transform: translateX(4px);
  transition: opacity 140ms ease, transform 140ms ease;
}

.block-shell:hover .block-hover-actions {
  opacity: 1;
  transform: translateX(0);
}

.block-action-btn {
  backdrop-filter: blur(4px);
}

.menu-anchor {
  display: inline-block;
  width: 1px;
  height: 1px;
}

.slash-menu {
  width: min(330px, 72vw);
  border: 1px solid var(--fox-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--fox-surface) 91%, black 9%);
  padding: 0.62rem;
  box-shadow: 0 15px 28px rgba(0, 0, 0, 0.34);
}

.slash-menu-title {
  margin: 0 0 0.45rem;
  font-size: 0.92rem;
  font-weight: 600;
  color: var(--fox-text-strong);
}

.slash-search {
  margin-bottom: 0.55rem;
}

.slash-options {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.slash-option {
  width: 100%;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--fox-text-body);
  text-align: left;
  padding: 0.42rem 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.08rem;
  cursor: pointer;
}

.slash-option:hover {
  background: var(--fox-chip);
}

.slash-option-label {
  font-family: "Iosevka", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  color: var(--fox-text-strong);
}

.slash-option-desc {
  color: var(--fox-text-muted);
  font-size: 0.82rem;
}

.empty {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: flex-start;
  gap: 0.2rem;
}

.empty h2 {
  font-size: 1.25rem;
}

.empty p {
  margin: 0;
  color: var(--fox-text-muted);
}

@media (max-width: 1080px) {
  .block-hover-actions {
    position: static;
    opacity: 1;
    transform: none;
    flex-direction: row;
    margin-bottom: 0.3rem;
  }
}
</style>
