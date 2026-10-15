import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ parent }) => {
  const { breadcrumbs, ranking } = await parent();
  return {
    title: `${ranking.title} combinations`,
    breadcrumbs: [...breadcrumbs, { label: "Rank Info" }],
  };
};
