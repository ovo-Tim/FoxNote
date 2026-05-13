import { describe, expect, it } from "vitest";
import { splitTypstContentIntoBlocks } from "./typstHeadingSplit";

describe("splitTypstContentIntoBlocks", () => {
  it("does not convert == lines inside fenced code blocks into title blocks", () => {
    const input = [
      "Intro paragraph",
      "```typst",
      "== should stay code",
      "#set text(fill: rgb(\"#fff\"))",
      "```",
      "== Real heading",
      "Body after heading",
    ].join("\n");

    const blocks = splitTypstContentIntoBlocks(input);
    expect(blocks).not.toBeNull();
    expect(blocks).toHaveLength(3);

    expect(blocks?.[0]).toMatchObject({
      type: "typst",
      content: ["Intro paragraph", "```typst", "== should stay code", "#set text(fill: rgb(\"#fff\"))", "```"].join(
        "\n",
      ),
    });

    expect(blocks?.[1]).toMatchObject({
      type: "title",
      level: 2,
      content: "Real heading",
    });

    expect(blocks?.[2]).toMatchObject({
      type: "typst",
      content: "Body after heading",
    });
  });
});
