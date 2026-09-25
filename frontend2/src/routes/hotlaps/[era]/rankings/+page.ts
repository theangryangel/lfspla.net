import { getList, type RankingSummary } from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, params, fetch, parent }) => {
  depends("app:hotlaps");
  const { me, era } = await parent();
  if (!me.authenticated || !era.rankings.length) return { progress: null };

  const rankings = await getList<RankingSummary>(
    fetch,
    `/api/v1/eras/${encodeURIComponent(params.era)}/rankings`,
  );
  return {
    progress: Object.fromEntries(
      rankings.map((ranking) => [
        ranking.id,
        {
          completed: ranking.my_progress?.completed_combinations ?? 0,
          total: ranking.my_progress?.total_combinations ?? 0,
        },
      ]),
    ),
  };
};
