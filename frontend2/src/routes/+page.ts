import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const stats = await api.stats.getStats().catch(() => null);
  return { stats };
};
