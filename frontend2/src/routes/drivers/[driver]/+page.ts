import { get, type PlayerResponse } from "$lib/api.js";
import type { PageLoad } from "./$types";

/** Loads one driver's public profile: lifetime totals, per-era record and highlights. */
export const load: PageLoad = async ({ depends, params, fetch, parent }) => {
  depends("app:hotlaps");
  const player = await get<PlayerResponse>(
    fetch,
    `/api/v1/players/${encodeURIComponent(params.driver)}`,
  );
  const { breadcrumbs } = await parent();
  return {
    player,
    breadcrumbs: [
      ...breadcrumbs,
      {
        label: player.display_name,
        flag: player.country_code,
        href: `/drivers/${encodeURIComponent(params.driver)}`,
      },
    ],
  };
};
