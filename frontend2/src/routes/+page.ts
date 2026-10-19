import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const [stats, communityCards] = await Promise.all([
    api.stats.getStats().catch(() => null),
    api.communityCards
      .listCommunityCards()
      .then((response) => response.items)
      .catch(() => null),
  ]);
  return { stats, communityCards };
};
