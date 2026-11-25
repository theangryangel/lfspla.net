import type { PageLoad } from "./$types";
export const load: PageLoad = async ({ parent }) => {
  const { breadcrumbs } = await parent();
  return {
    title: "Create team",
    breadcrumbs: [...breadcrumbs, { label: "Create team", href: "/teams/new" }],
  };
};
