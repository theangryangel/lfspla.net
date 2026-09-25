/**
 * One step of the trail above the current page.
 *
 * Routes build these in their `load` and append them to the trail their parent
 * returned, so the crumbs follow the route tree without any one file having to
 * know the shape of the whole app. A crumb always carries its own `href`: the
 * renderer drops it from the last crumb, which means a route never has to work
 * out whether it happens to be the deepest one.
 */
export type Crumb = {
  label: string;
  href?: string;
  /** Country code drawn as a flag before the label. */
  flag?: string | null;
  /**
   * Names a handler the renderer supplies, for a level that is a dialog rather
   * than a page. A crumb whose action the renderer does not know falls back to
   * plain text, so a trail is never left with a dead control.
   */
  action?: string;
  /** Small badge drawn after the label. */
  tag?: { label: string; accent?: boolean };
};
