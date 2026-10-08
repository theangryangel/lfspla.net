import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, parent }) => {
  const api = createApi(fetch);
  const { me, breadcrumbs } = await parent();
  return {
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Personal settings", href: "/account/personal" },
    ],
    countries: me.authenticated
      ? await api.countries.listCountries().then((response) => response.items)
      : [],
  };
};
