import { describe, expect, it } from "vitest";
import { shouldIgnoreBlockFocusClick, shouldSkipMarqueeStart } from "./interactionTargets";

function appendFixture(html: string): HTMLElement {
  const container = document.createElement("div");
  container.innerHTML = html;
  document.body.appendChild(container);
  return container;
}

describe("shouldSkipMarqueeStart", () => {
  it("returns true for regular html links", () => {
    const fixture = appendFixture('<div class="preview"><a id="link" href="https://example.com">go</a></div>');
    const target = fixture.querySelector("#link") as Element | null;
    expect(shouldSkipMarqueeStart(target)).toBe(true);
    fixture.remove();
  });

  it("returns true for svg links using xlink:href", () => {
    const fixture = appendFixture(
      '<svg><a id="svg-link" xlink:href="https://example.com"><text>go</text></a></svg>',
    );
    const textNode = fixture.querySelector("text") as Element | null;
    expect(shouldSkipMarqueeStart(textNode)).toBe(true);
    fixture.remove();
  });

  it("returns true for svg links using href", () => {
    const fixture = appendFixture(
      '<svg><a id="svg-link" href="https://example.com"><text id="svg-text">go</text></a></svg>',
    );
    const textNode = fixture.querySelector("#svg-text") as Element | null;
    expect(shouldSkipMarqueeStart(textNode)).toBe(true);
    fixture.remove();
  });

  it("returns false for plain non-interactive blocks", () => {
    const fixture = appendFixture('<div id="plain">plain</div>');
    const target = fixture.querySelector("#plain") as Element | null;
    expect(shouldSkipMarqueeStart(target)).toBe(false);
    fixture.remove();
  });
});

describe("shouldIgnoreBlockFocusClick", () => {
  it("returns true for regular html links", () => {
    const fixture = appendFixture('<div class="preview"><a id="link" href="https://example.com">go</a></div>');
    const target = fixture.querySelector("#link") as Element | null;
    expect(shouldIgnoreBlockFocusClick(target)).toBe(true);
    fixture.remove();
  });

  it("returns true for nested svg text inside link anchor", () => {
    const fixture = appendFixture(
      '<svg><a xlink:href="https://example.com"><text id="svg-text">go</text></a></svg>',
    );
    const target = fixture.querySelector("#svg-text") as Element | null;
    expect(shouldIgnoreBlockFocusClick(target)).toBe(true);
    fixture.remove();
  });

  it("returns true for typst pseudo-link rectangles", () => {
    const fixture = appendFixture(
      '<svg><a xlink:href="https://example.com"><rect id="pseudo" class="pseudo-link" width="100" height="20"></rect></a></svg>',
    );
    const target = fixture.querySelector("#pseudo") as Element | null;
    expect(shouldIgnoreBlockFocusClick(target)).toBe(true);
    fixture.remove();
  });

  it("returns false for typst foreignObject text selection layer", () => {
    const fixture = appendFixture('<svg><foreignObject id="fo"><div class="tsel">text</div></foreignObject></svg>');
    const target = fixture.querySelector(".tsel") as Element | null;
    expect(shouldIgnoreBlockFocusClick(target)).toBe(false);
    fixture.remove();
  });

  it("returns false for plain non-interactive blocks", () => {
    const fixture = appendFixture('<div id="plain">plain</div>');
    const target = fixture.querySelector("#plain") as Element | null;
    expect(shouldIgnoreBlockFocusClick(target)).toBe(false);
    fixture.remove();
  });
});
