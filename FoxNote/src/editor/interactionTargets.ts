const INTERACTION_CAPTURE_BYPASS_SELECTOR = [
  "button",
  "input",
  "textarea",
  "[contenteditable='true']",
  ".v-field",
  ".block-hover-actions",
  ".tag-menu",
  ".slash-menu",
  ".action-menu",
  ".block-resize-handle",
  ".image-paint-zone",
  ".vp-editor",
  ".vp-main",
  ".vp-image",
  ".vp-toolbar",
  ".tldraw-zone",
  ".tl-container",
  ".tlui-layout",
].join(", ");

function hasLinkBehavior(target: Element): boolean {
  const tag = target.tagName.toLowerCase();
  if (tag === "a") {
    return true;
  }

  if (target.hasAttribute("href") || target.getAttribute("xlink:href") !== null) {
    return true;
  }

  const xlinkHref = target.getAttributeNS("http://www.w3.org/1999/xlink", "href");
  return xlinkHref !== null;
}

function isOrContainsInteractiveLink(target: Element): boolean {
  let cursor: Element | null = target;
  while (cursor) {
    if (hasLinkBehavior(cursor)) {
      return true;
    }
    cursor = cursor.parentElement;
  }

  return false;
}

export function shouldSkipMarqueeStart(target: Element | null): boolean {
  if (!target) {
    return false;
  }

  return isOrContainsInteractiveLink(target) || Boolean(target.closest(INTERACTION_CAPTURE_BYPASS_SELECTOR));
}

export function shouldIgnoreBlockFocusClick(target: Element | null): boolean {
  if (!target) {
    return false;
  }

  return isOrContainsInteractiveLink(target) || Boolean(target.closest(INTERACTION_CAPTURE_BYPASS_SELECTOR));
}
