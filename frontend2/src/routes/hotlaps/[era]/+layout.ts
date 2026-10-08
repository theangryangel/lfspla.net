import { error } from "@sveltejs/kit";
import type { LayoutLoad } from "./$types";

export const load: LayoutLoad = async ({ params, parent }) => {
  const { eras, breadcrumbs } = await parent();
  const era = eras.find((candidate) => candidate.id === params.era);
  if (!era) error(404, "Era not found");
  return {
    era,
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Hotlaps" },
      {
        label: era.title,
        href: `/hotlaps/${era.id}`,
        tag: {
          label: era.open ? "Open" : "Historical",
          accent: era.open,
        },
      },
    ],
  };
};
