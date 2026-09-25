import {
  get,
  getList,
  type HotlapActivityResponse,
  type StatsResponse,
} from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch }) => {
  depends("app:hotlaps");
  const [activity, stats] = await Promise.all([
    getList<HotlapActivityResponse>(
      fetch,
      "/api/v1/hotlaps?state=valid&per_page=10",
    )
      .then((activity) => ({ activity, activityUnavailable: false }))
      .catch(() => ({ activity: [], activityUnavailable: true })),
    get<StatsResponse>(fetch, "/api/v1/stats").catch(() => null),
  ]);
  return { ...activity, stats };
};
