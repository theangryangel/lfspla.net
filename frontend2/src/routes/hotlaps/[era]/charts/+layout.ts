import { createApi } from "$lib/api.js";

import type { LayoutLoad } from "./$types";

export const load: LayoutLoad = async ({ fetch, parent }) => {
  const api = createApi(fetch);
  const countries = api.countries
    .listCountries()
    .then((response) => response.items);
  const { breadcrumbs } = await parent();
  return {
    countries: await countries,
    // The picker is a dialog rather than a page, so this crumb opens it where
    // the era layout renders the trail, and is plain text anywhere else.
    breadcrumbs: [...breadcrumbs, { label: "Charts", action: "charts" }],
  };
};
