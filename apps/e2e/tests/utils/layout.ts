import { expect, type Page } from "@playwright/test";
import { expectNoAxeViolations } from "./web";

/*
 * Text-overflow check without screenshots: at a phone width and with the browser's text size
 * doubled, nothing a person must read or press may be cut off or pushed out of reach. The page
 * measures itself from bounding boxes and computed styles; Playwright only reports what it finds.
 */

/** The narrowest phone layout the screens promise to support (alpha-ux-flows.md section 6.1). */
export const NARROW_VIEWPORT = { width: 320, height: 640 } as const;
/** 200 % text size, as the browser's font-size setting scales the root size the screens use. */
export const LARGE_TEXT = "200%";

/** What `fitProblems` reports: one line per element that does not fit. */
export type FitProblem = string;

/**
 * Lists every problem on the current screen:
 * - the page scrolls sideways;
 * - a piece of text or a control lies (partly) outside the page's width;
 * - an element clips its own text (overflow hidden, ellipsis) or lies outside an ancestor that
 *   clips it;
 * - an element inside a fixed layer (night screen, full-screen alerts, dialogs) ends below or
 *   right of the viewport, where nothing can scroll it into view.
 * Scrolling vertically on the page or inside an `overflow: auto` region is fine.
 */
export async function fitProblems(page: Page): Promise<FitProblem[]> {
  return page.evaluate(() => {
    const problems: string[] = [];
    const root = document.documentElement;
    const width = root.clientWidth;
    const height = window.innerHeight;
    const SLACK = 1.5;
    const name = (el: Element): string => {
      const id = el.closest("[data-testid]")?.getAttribute("data-testid");
      const own = el.getAttribute("data-testid");
      const label = (el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 30);
      const cls =
        typeof el.className === "string" && el.className ? `.${el.className.split(" ")[0]}` : "";
      return `${el.tagName.toLowerCase()}${cls}${own ? `[${own}]` : id ? ` in [${id}]` : ""} "${label}"`;
    };
    const scrolls = (value: string): boolean => value === "auto" || value === "scroll";
    const clips = (value: string): boolean => value === "hidden" || value === "clip";

    if (root.scrollWidth > width + SLACK) {
      // Name the widest offenders, text or not, so the report points at the cause.
      const wide = Array.from(document.body.querySelectorAll("*"))
        .filter(
          (el) =>
            el.getBoundingClientRect().right > width + SLACK ||
            (el.clientWidth > 0 && el.scrollWidth > el.clientWidth + SLACK),
        )
        .slice(0, 5)
        .map(name);
      problems.push(
        `page scrolls sideways (${root.scrollWidth}px > ${width}px): ${wide.join("; ") || "no single element sticks out"}`,
      );
    }

    for (const el of Array.from(document.body.querySelectorAll("*"))) {
      if (el.closest(".gs-skip")) continue; // off-screen until focused, on purpose
      const style = getComputedStyle(el);
      if (style.display === "none" || style.visibility === "hidden") continue;
      const hasText = Array.from(el.childNodes).some(
        (n) => n.nodeType === Node.TEXT_NODE && (n.textContent ?? "").trim() !== "",
      );
      const isControl = el.matches("button, input, select, textarea, a[href], summary");
      if (!hasText && !isControl) continue;
      const rect = el.getBoundingClientRect();
      // Visually hidden helpers (1 px boxes) are for screen readers only.
      if (rect.width <= 1 || rect.height <= 1) continue;

      if (rect.left < -SLACK || rect.right > width + SLACK) {
        problems.push(
          `${name(el)} leaves the page width (${Math.round(rect.left)}..${Math.round(rect.right)} of ${width})`,
        );
      }
      if (hasText) {
        if (clips(style.overflowX) && el.scrollWidth > el.clientWidth + SLACK) {
          problems.push(`${name(el)} clips its text sideways`);
        }
        if (clips(style.overflowY) && el.scrollHeight > el.clientHeight + SLACK) {
          problems.push(`${name(el)} clips its text vertically`);
        }
      }

      let inFixedLayer = false;
      for (let node: Element | null = el; node && node !== root; node = node.parentElement) {
        const s = getComputedStyle(node);
        if (node !== el) {
          const box = node.getBoundingClientRect();
          if (
            clips(s.overflowX) &&
            (rect.left < box.left - SLACK || rect.right > box.right + SLACK)
          ) {
            problems.push(`${name(el)} is cut off sideways by ${name(node)}`);
          }
          if (
            clips(s.overflowY) &&
            (rect.top < box.top - SLACK || rect.bottom > box.bottom + SLACK)
          ) {
            problems.push(`${name(el)} is cut off vertically by ${name(node)}`);
          }
        }
        if (s.position === "fixed") {
          // A fixed layer scrolls only when it says so; otherwise what sticks out is lost.
          const reachable = scrolls(s.overflowY);
          if (!reachable) inFixedLayer = true;
        }
      }
      if (inFixedLayer && (rect.bottom > height + SLACK || rect.right > width + SLACK)) {
        problems.push(
          `${name(el)} lies outside the screen in a fixed layer (bottom ${Math.round(rect.bottom)} of ${height})`,
        );
      }
    }
    return Array.from(new Set(problems));
  });
}

/** Waits until finite animations and transitions (dialog slide-in, fade-in) have ended. */
export async function settle(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const finite = document
      .getAnimations()
      .filter((a) => a.effect?.getComputedTiming().iterations !== Infinity);
    await Promise.allSettled(finite.map((a) => a.finished));
  });
}

/**
 * The accessibility and text-fit check of one screen: axe at the normal size first, then the
 * overflow check at 320 px width with text at 200 %, then back to the normal size.
 */
export async function checkScreen(page: Page, screen: string): Promise<void> {
  await settle(page);
  await expectNoAxeViolations(page);

  const normal = page.viewportSize();
  await page.setViewportSize(NARROW_VIEWPORT);
  await page.evaluate((size) => {
    document.documentElement.style.fontSize = size;
  }, LARGE_TEXT);
  try {
    await settle(page);
    // The axe run catches reflow and contrast problems that only show in the narrow layout.
    await expectNoAxeViolations(page);
    expect(await fitProblems(page), `${screen} at 320 px and 200 % text`).toEqual([]);
  } finally {
    await page.evaluate(() => {
      document.documentElement.style.fontSize = "";
    });
    if (normal) await page.setViewportSize(normal);
    await settle(page);
  }
}
