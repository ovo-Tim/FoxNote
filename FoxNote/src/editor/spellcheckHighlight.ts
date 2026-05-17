import type { SpellcheckIssue } from "./spellcheck";

export interface SpellcheckHighlightSegment {
  key: string;
  text: string;
  issue: SpellcheckIssue | null;
}

export interface SpellcheckPopoverRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface SpellcheckViewportSize {
  width: number;
  height: number;
}

export interface SpellcheckPopoverSize {
  width: number;
  height: number;
}

export interface SpellcheckPopoverPlacement {
  left: number;
  top: number;
  placement: "top" | "bottom";
}

export function buildSpellcheckHighlightSegments(
  text: string,
  issues: SpellcheckIssue[],
): SpellcheckHighlightSegment[] {
  if (!text.length || !issues.length) {
    return [
      {
        key: "plain:0",
        text,
        issue: null,
      },
    ];
  }

  const orderedIssues = [...issues]
    .filter((issue) => issue.end > issue.start)
    .sort((left, right) => left.start - right.start || left.end - right.end);

  const segments: SpellcheckHighlightSegment[] = [];
  let cursor = 0;

  for (const issue of orderedIssues) {
    const start = Math.max(0, Math.min(text.length, issue.start));
    const end = Math.max(start, Math.min(text.length, issue.end));
    if (end <= cursor) {
      continue;
    }

    if (start > cursor) {
      segments.push({
        key: `plain:${cursor}`,
        text: text.slice(cursor, start),
        issue: null,
      });
    }

    segments.push({
      key: issue.id,
      text: text.slice(start, end),
      issue,
    });
    cursor = end;
  }

  if (cursor < text.length) {
    segments.push({
      key: `plain:${cursor}`,
      text: text.slice(cursor),
      issue: null,
    });
  }

  return segments.length
    ? segments
    : [
      {
        key: "plain:0",
        text,
        issue: null,
      },
    ];
}

export function computeSpellcheckPopoverPlacement(
  rect: SpellcheckPopoverRect,
  viewport: SpellcheckViewportSize,
  popup: SpellcheckPopoverSize,
): SpellcheckPopoverPlacement {
  const gutter = 12;
  const gap = -30;
  const placement = rect.top >= popup.height + gap + gutter ? "top" : "bottom";
  const unclampedLeft = rect.left;
  const maxLeft = Math.max(gutter, viewport.width - popup.width - gutter);

  return {
    left: Math.min(Math.max(unclampedLeft, gutter), maxLeft),
    top: placement === "top" ? rect.top - gap : rect.top + rect.height + gap,
    placement,
  };
}
