import { describe, expect, it, vi } from "vitest";
import { applySpellcheckSuggestion, createSpellcheckLintOptions, TYPST_FORMULA_REGEX_MASK } from "./spellcheck";

describe("spellcheck helpers", () => {
  it("uses Typst mode with formula masking", () => {
    expect(createSpellcheckLintOptions("typst")).toEqual({
      language: "typst",
      regex_mask: TYPST_FORMULA_REGEX_MASK,
    });
  });

  it("uses plaintext mode for non-Typst fields", () => {
    expect(createSpellcheckLintOptions("plaintext")).toEqual({
      language: "plaintext",
    });
  });

  it("delegates suggestion application through the linter", async () => {
    const applySuggestion = vi.fn(async (text: string) => text.replace("teh", "the"));
    const linter = {
      setup: vi.fn(async () => undefined),
      lint: vi.fn(async () => []),
      applySuggestion,
    };
    const lint = { id: "lint" };
    const suggestion = { id: "suggestion" };

    await expect(applySpellcheckSuggestion(linter, "teh sentence", lint, suggestion)).resolves.toBe("the sentence");
    expect(applySuggestion).toHaveBeenCalledWith("teh sentence", lint, suggestion);
  });
});
