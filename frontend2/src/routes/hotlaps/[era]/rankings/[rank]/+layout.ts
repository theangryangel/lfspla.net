import { get, type RankingDetailResponse } from "$lib/api.js";
import { hotlapPath } from "$lib/era.js";
import type { LayoutLoad } from "./$types";

/** Loads the ranking's rules and the charts it requires. */
export const load: LayoutLoad = async ({ depends, params, fetch, parent }) => {
  depends("app:hotlaps");
  const ranking = await get<RankingDetailResponse>(
    fetch,
    `/api/v1/eras/${encodeURIComponent(params.era)}/rankings/${encodeURIComponent(params.rank)}`,
  );
  const { breadcrumbs, hotlapActions = [] } = await parent();
  return {
    ranking,
    hotlapActions: [
      ...hotlapActions,
      {
        label: "Rank Info",
        href: `${hotlapPath(params.era)}/rankings/${ranking.id}/combinations`,
      },
    ],
    breadcrumbs: [
      ...breadcrumbs,
      {
        label: ranking.title,
        href: `${hotlapPath(params.era)}/rankings/${ranking.id}`,
      },
    ],
  };
};
