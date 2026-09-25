import { getList, type CountrySummary } from "$lib/api.js";
import type { LayoutLoad } from "./$types";

export const load: LayoutLoad = async ({ fetch, parent }) => {
  const countries = getList<CountrySummary>(fetch, "/api/v1/countries");
  const { breadcrumbs } = await parent();
  return {
    countries: await countries,
    // The picker is a dialog rather than a page, so this crumb opens it where
    // the era layout renders the trail, and is plain text anywhere else.
    breadcrumbs: [...breadcrumbs, { label: "Charts", action: "charts" }],
  };
};
