import { getMyHotlaps, submissionQuery } from "$lib/hotlaps.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch, parent, url }) => {
  depends("app:hotlaps");
  const { me, breadcrumbs } = await parent();
  const trail = [
    ...breadcrumbs,
    { label: "My hotlaps", href: "/account/hotlaps" },
  ];
  if (!me.authenticated) return { hotlaps: null, breadcrumbs: trail };
  return {
    hotlaps: await getMyHotlaps(fetch, { ...submissionQuery(url) }),
    breadcrumbs: trail,
  };
};
