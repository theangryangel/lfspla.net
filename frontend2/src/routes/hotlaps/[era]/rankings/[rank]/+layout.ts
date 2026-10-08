import { createApi } from "$lib/api.js";

import type { LayoutLoad } from "./$types";

/** Loads the ranking's rules and the charts it requires. */
export const load: LayoutLoad = async ({ depends, params, fetch, parent }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const ranking = await api.ranking.getEraRanking({
    era: params.era,
    ranking: params.rank,
  });
  const { breadcrumbs } = await parent();
  return {
    ranking,
    breadcrumbs: [
      ...breadcrumbs,
      {
        label: ranking.title,
        href: `/hotlaps/${params.era}/rankings/${ranking.id}`,
      },
    ],
  };
};
