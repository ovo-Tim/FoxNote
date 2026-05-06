import { describe, expect, it } from "vitest";
import { useTagFilter } from "./useTagFilter";

describe("useTagFilter", () => {
  it("filters note lists by selected tag note ids", () => {
    const filter = useTagFilter();

    filter.setTags([
      { path: "work/project", name: "project", noteCount: 2 },
      { path: "personal", name: "personal", noteCount: 1 },
    ]);

    filter.setActiveTag("work/project", ["n1", "n3"]);

    const list = filter.filteredNotes.value([
      {
        id: "n1",
        folder: "work",
        title: "Sprint",
        date: "2026-05-05",
        type: "notes",
        tags: ["work/project"],
      },
      {
        id: "n2",
        folder: "work",
        title: "Retro",
        date: "2026-05-05",
        type: "notes",
        tags: ["work/project"],
      },
      {
        id: "n3",
        folder: "personal",
        title: "Reading",
        date: "2026-05-05",
        type: "notes",
        tags: ["personal"],
      },
    ]);

    expect(list.map((item) => item.id)).toEqual(["n1", "n3"]);
    expect(filter.activeTagLabel.value).toBe("work/project (2)");

    filter.clearFilter();
    expect(filter.isFiltering.value).toBe(false);
  });
});
