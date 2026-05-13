import { describe, expect, it } from "vitest";
import { resolvePreviewLinkUrl } from "./previewLinks";

function appendFixture(html: string): HTMLElement {
  const container = document.createElement("div");
  container.innerHTML = html;
  document.body.appendChild(container);
  return container;
}

describe("resolvePreviewLinkUrl", () => {
  it("returns direct html anchor href from the target ancestry", () => {
    const fixture = appendFixture('<div id="scope"><a href="https://example.com"><span id="target">go</span></a></div>');
    const target = fixture.querySelector("#target");
    const scope = fixture.querySelector("#scope");

    expect(
      resolvePreviewLinkUrl({
        target,
        clientX: 0,
        clientY: 0,
        scope,
        elementsFromPoint: () => [],
      }),
    ).toBe("https://example.com");

    fixture.remove();
  });

  it("resolves typst pseudo-link anchor from elementsFromPoint when click target is foreignObject text", () => {
    const fixture = appendFixture(`
      <div id="scope">
        <svg>
          <a id="hotspot" xlink:href="https://example.com/path">
            <rect class="pseudo-link" width="100" height="20"></rect>
          </a>
          <foreignObject>
            <div class="tsel" id="text-layer">Visible URL</div>
          </foreignObject>
        </svg>
      </div>
    `);

    const target = fixture.querySelector("#text-layer");
    const scope = fixture.querySelector("#scope");
    const hotspot = fixture.querySelector("#hotspot") as Element;

    expect(
      resolvePreviewLinkUrl({
        target,
        clientX: 12,
        clientY: 34,
        scope,
        elementsFromPoint: () => [target as Element, hotspot],
      }),
    ).toBe("https://example.com/path");

    fixture.remove();
  });

  it("ignores anchors outside the provided scope", () => {
    const fixture = appendFixture(`
      <div>
        <div id="scope"><span id="target">text</span></div>
        <a id="outside" href="https://example.com/outside">outside</a>
      </div>
    `);
    const target = fixture.querySelector("#target");
    const scope = fixture.querySelector("#scope");
    const outside = fixture.querySelector("#outside") as Element;

    expect(
      resolvePreviewLinkUrl({
        target,
        clientX: 0,
        clientY: 0,
        scope,
        elementsFromPoint: () => [outside],
      }),
    ).toBeNull();

    fixture.remove();
  });
});
