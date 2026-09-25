import { getList, type CountrySummary } from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, parent }) => {
  const { me, breadcrumbs } = await parent();
  return {
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Personal settings", href: "/account/personal" },
    ],
    countries: me.authenticated
      ? await getList<CountrySummary>(fetch, "/api/v1/countries")
      : [],
  };
};
