import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

/** Loads one driver's public profile: lifetime totals, per-era record and highlights. */
export const load: PageLoad = async ({ depends, params, fetch, parent }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const player = await api.players.getPlayer({ lfs_username: params.driver });
  const { breadcrumbs } = await parent();
  return {
    player,
    breadcrumbs: [
      ...breadcrumbs,
      {
        label: player.display_name,
        flag: player.flag_code ?? player.country_code,
        href: `/drivers/${encodeURIComponent(params.driver)}`,
      },
    ],
  };
};
