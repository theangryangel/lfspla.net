import { getMyHotlaps, submissionQuery } from "$lib/hotlaps.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({
  depends,
  fetch,
  parent,
  url,
  params,
}) => {
  depends("app:hotlaps");
  const { me, breadcrumbs } = await parent();
  const trail = [...breadcrumbs, { label: "My submissions" }];
  if (!me.authenticated) return { hotlaps: null, breadcrumbs: trail };
  return {
    hotlaps: await getMyHotlaps(fetch, {
      ...submissionQuery(url),
      era_id: params.era,
    }),
    breadcrumbs: trail,
  };
};
