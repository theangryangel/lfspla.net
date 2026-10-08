import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch, parent, url }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const { era } = await parent();
  const holders = await api.ranking.listEraWorldRecordHolders({
    era: era.id,
    page: url.searchParams.get("wr_page")
      ? Number(url.searchParams.get("wr_page"))
      : undefined,
    per_page: 25,
  });
  return {
    holders: holders.items,
    pagination: holders.pagination,
  };
};
