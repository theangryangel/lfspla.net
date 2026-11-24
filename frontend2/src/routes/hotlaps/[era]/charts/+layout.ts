import { createApi } from "#lib/api.js";

import type { LayoutLoad } from "./$types";

export const load: LayoutLoad = async ({ fetch, parent }) => {
  const api = createApi(fetch);
  const countries = api.countries
    .listCountries()
    .then((response) => response.items);
  const { breadcrumbs } = await parent();
  return {
    countries: await countries,
    // The era layout sends this action to the last valid chart.
    breadcrumbs: [...breadcrumbs, { label: "Charts", action: "charts" }],
  };
};
