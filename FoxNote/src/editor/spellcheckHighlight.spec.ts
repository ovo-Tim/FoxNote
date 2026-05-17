import { describe, expect, it } from "vitest";
import type { SpellcheckIssue } from "./spellcheck";
import { buildSpellcheckHighlightSegments, computeSpellcheckPopoverPlacement } from "./spellcheckHighlight";

function createIssue(overrides: Partial<SpellcheckIssue>): SpellcheckIssue {
  return {
    id: overrides.id ?? "issue-1",
    start: overrides.start ?? 0,
    end: overrides.end ?? 1,
    message: overrides.message ?? "Message",
    kind: overrides.kind ?? "Spelling",
    problemText: overrides.problemText ?? "oops",
    suggestions: overrides.suggestions ?? [],
    lint: overrides.lint ?? ({} as SpellcheckIssue["lint"]),
  };
}

describe("buildSpellcheckHighlightSegments", () => {
  it("splits plain text around issue spans", () => {
    const issue = createIssue({ id: "a", start: 6, end: 12, problemText: "Hellow" });
    const segments = buildSpellcheckHighlightSegments("Hello Hellow world", [issue]);

    expect(segments.map((segment) => segment.text)).toEqual(["Hello ", "Hellow", " world"]);
    expect(segments[1]?.issue?.id).toBe("a");
  });

  it("returns a single plain segment without issues", () => {
    const segments = buildSpellcheckHighlightSegments("Clean text", []);

    expect(segments).toEqual([{ key: "plain:0", text: "Clean text", issue: null }]);
  });
});

describe("computeSpellcheckPopoverPlacement", () => {
  it("places popup below when there is room", () => {
    const placement = computeSpellcheckPopoverPlacement(
      { left: 120, top: 40, width: 50, height: 18 },
      { width: 900, height: 700 },
      { width: 300, height: 160 },
    );

    expect(placement.placement).toBe("bottom");
    expect(placement.top).toBe(28);
  });

  it("flips popup above when there is not enough room below", () => {
    const placement = computeSpellcheckPopoverPlacement(
      { left: 120, top: 320, width: 50, height: 18 },
      { width: 900, height: 700 },
      { width: 300, height: 160 },
    );

    expect(placement.placement).toBe("top");
    expect(placement.top).toBe(350);
  });
});
