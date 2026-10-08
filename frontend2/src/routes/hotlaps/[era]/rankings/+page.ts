import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, params, fetch, parent }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const { me, era } = await parent();
  if (!me.authenticated || !era.rankings.length) return { progress: null };

  const rankings = await api.ranking
    .listEraRankings({ era: params.era })
    .then((response) => response.items);
  return {
    progress: Object.fromEntries(
      rankings.map((ranking) => [
        ranking.id,
        {
          completed: ranking.my_progress?.completed_charts ?? 0,
          total: ranking.my_progress?.total_charts ?? 0,
        },
      ]),
    ),
  };
};
