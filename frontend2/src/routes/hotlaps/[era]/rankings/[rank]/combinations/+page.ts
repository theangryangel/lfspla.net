import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ parent }) => {
  const { breadcrumbs } = await parent();
  return { breadcrumbs: [...breadcrumbs, { label: "Rank Info" }] };
};
