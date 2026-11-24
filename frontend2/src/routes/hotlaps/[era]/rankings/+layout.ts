import type { LayoutLoad } from "./$types";

/** Carries the "Rankings" crumb, so every page below a ranking inherits it. */
export const load: LayoutLoad = async ({ parent }) => {
  const { breadcrumbs, era } = await parent();
  return {
    breadcrumbs: [
      ...breadcrumbs,
      {
        label: "Rankings",
        href: `/hotlaps/${encodeURIComponent(era.id)}/rankings`,
      },
    ],
  };
};
