import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ parent }) => {
  const { breadcrumbs, ranking } = await parent();
  return {
    title: `${ranking.title} — rank info & progress`,
    breadcrumbs: [...breadcrumbs, { label: "Rank info & progress" }],
  };
};
