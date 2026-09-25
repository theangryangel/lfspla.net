import { error, redirect } from "@sveltejs/kit";
import { hotlapPath } from "$lib/era.js";
import type { LayoutLoad } from "./$types";

export const load: LayoutLoad = async ({ params, url, fetch, parent }) => {
  const { currentEraId, eras, breadcrumbs } = await parent();
  // `current` is an alias only: canonical URLs keep a concrete era id so saved
  // links do not drift when the current era changes.
  if (params.era === "current") {
    if (!currentEraId) error(503, "No eras are configured yet");
    redirect(
      307,
      url.pathname.replace("/current", `/${currentEraId}`) + url.search,
    );
  }

  const era = eras.find((candidate) => candidate.id === params.era);
  if (!era) error(404, "Era not found");
  return {
    era,
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Hotlaps" },
      {
        label: era.title,
        href: hotlapPath(era.id),
        tag: {
          label: era.open ? "Open" : "Historical",
          accent: era.open,
        },
      },
    ],
  };
};
