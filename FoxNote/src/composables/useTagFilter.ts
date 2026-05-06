import { computed, ref } from "vue";
import type { NoteSummary, TagEntry } from "../types/note";

function isTagInBranch(noteTag: string, activeTag: string): boolean {
  return noteTag === activeTag || noteTag.startsWith(`${activeTag}/`);
}

export function useTagFilter() {
  const activeTag = ref<string>("");
  const tags = ref<TagEntry[]>([]);
  const taggedNoteIds = ref<Set<string>>(new Set());

  const isFiltering = computed(() => activeTag.value.length > 0);

  const filteredNotes = computed(() => {
    return (notes: NoteSummary[]): NoteSummary[] => {
      if (!isFiltering.value) {
        return notes;
      }

      return notes.filter((note) => taggedNoteIds.value.has(note.id));
    };
  });

  const activeTagLabel = computed(() => {
    if (!activeTag.value) {
      return "All notes";
    }
    const matched = tags.value.find((tag) => tag.path === activeTag.value);
    if (!matched) {
      return activeTag.value;
    }
    return `${matched.path} (${matched.noteCount})`;
  });

  function setTags(nextTags: TagEntry[]) {
    tags.value = [...nextTags].sort((left, right) => left.path.localeCompare(right.path));
  }

  function setActiveTag(tagPath: string, noteIds: string[]) {
    activeTag.value = tagPath;
    taggedNoteIds.value = new Set(noteIds);
  }

  function clearFilter() {
    activeTag.value = "";
    taggedNoteIds.value = new Set();
  }

  function countMatchesForNote(note: NoteSummary): number {
    if (!activeTag.value) {
      return note.tags.length;
    }
    return note.tags.filter((noteTag) => isTagInBranch(noteTag, activeTag.value)).length;
  }

  return {
    activeTag,
    activeTagLabel,
    clearFilter,
    countMatchesForNote,
    filteredNotes,
    isFiltering,
    setActiveTag,
    setTags,
    tags,
  };
}
