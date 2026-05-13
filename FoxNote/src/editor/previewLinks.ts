import { openUrl } from "@tauri-apps/plugin-opener";

const XLINK_NS = "http://www.w3.org/1999/xlink";

function readElementLinkHref(target: Element): string | null {
  const href =
    target.getAttribute("href") ??
    target.getAttribute("xlink:href") ??
    target.getAttributeNS(XLINK_NS, "href");

  if (!href) {
    return null;
  }

  const normalized = href.trim();
  return normalized.length > 0 ? normalized : null;
}

function findLinkHrefInAncestors(target: Element | null, scope: Element | null): string | null {
  let cursor: Element | null = target;
  while (cursor) {
    const href = readElementLinkHref(cursor);
    if (href) {
      return href;
    }

    if (scope && cursor === scope) {
      break;
    }

    cursor = cursor.parentElement;
  }

  return null;
}

interface ResolvePreviewLinkUrlOptions {
  target: EventTarget | null;
  clientX: number;
  clientY: number;
  scope: Element | null;
  elementsFromPoint?: (x: number, y: number) => Element[];
}

export function resolvePreviewLinkUrl({
  target,
  clientX,
  clientY,
  scope,
  elementsFromPoint,
}: ResolvePreviewLinkUrlOptions): string | null {
  const targetEl = target instanceof Element ? target : null;
  const directHref = findLinkHrefInAncestors(targetEl, scope);
  if (directHref) {
    return directHref;
  }

  const lookup =
    elementsFromPoint ??
    ((x: number, y: number) => {
      if (typeof document === "undefined" || typeof document.elementsFromPoint !== "function") {
        return [];
      }
      return document.elementsFromPoint(x, y);
    });

  for (const element of lookup(clientX, clientY)) {
    if (scope && element !== scope && !scope.contains(element)) {
      continue;
    }

    const href = findLinkHrefInAncestors(element, scope);
    if (href) {
      return href;
    }
  }

  return null;
}

export async function openPreviewLinkUrl(url: string): Promise<void> {
  try {
    await openUrl(url);
    return;
  } catch {
    if (typeof window !== "undefined") {
      window.open(url, "_blank", "noopener,noreferrer");
    }
  }
}
