import { hotlapPath } from "$lib/era.js";
import type { LayoutLoad } from "./$types";

/** Carries the "Ranks" crumb, so every page below a ranking inherits it. */
export const load: LayoutLoad = async ({ params, parent }) => {
  const { breadcrumbs } = await parent();
  return {
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Ranks", href: `${hotlapPath(params.era)}/rankings` },
    ],
  };
};
