import { createApi } from "#lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ parent, fetch, depends }) => {
  depends("app:hotlaps");
  const { era } = await parent();
  const rankings = await createApi(fetch).ranking.listEraRankings({
    era: era.id,
  });
  return { rankings: rankings.items };
};
