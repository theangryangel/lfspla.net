import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch, parent }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const { era } = await parent();
  const holders = await api.ranking.listEraWorldRecordHolders({
    era: era.id,
    page: 1,
    per_page: 100,
  });
  return {
    leaders: holders.items.filter((holder) => holder.position === 1),
    holders: holders.items,
    pagination: holders.pagination,
  };
};
