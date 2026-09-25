import {
  get,
  query,
  type HotlapActivityResponse,
  type PaginatedResponse,
  type WorldRecordHolderResponse,
} from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch, parent, url }) => {
  depends("app:hotlaps");
  const { era } = await parent();
  const [activity, holders] = await Promise.all([
    get<PaginatedResponse<HotlapActivityResponse>>(
      fetch,
      "/api/v1/hotlaps" +
        query({
          era_id: era.id,
          state: "valid",
          mine: url.searchParams.get("mine"),
          column: url.searchParams.get("column"),
          order: url.searchParams.get("order"),
          page: url.searchParams.get("page"),
          per_page: 25,
        }),
    ),
    get<PaginatedResponse<WorldRecordHolderResponse>>(
      fetch,
      `/api/v1/eras/${encodeURIComponent(era.id)}/world-records` +
        query({
          page: url.searchParams.get("wr_page"),
          per_page: 25,
        }),
    ),
  ]);
  return {
    activity: activity.items,
    activityPagination: activity.pagination,
    holders: holders.items,
    pagination: holders.pagination,
  };
};
