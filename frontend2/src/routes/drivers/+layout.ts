import type { LayoutLoad } from "./$types";

/** Carries the "Drivers" crumb for the directory and every profile below it. */
export const load: LayoutLoad = async ({ parent }) => {
  const { breadcrumbs } = await parent();
  return {
    breadcrumbs: [...breadcrumbs, { label: "Drivers", href: "/drivers" }],
  };
};
