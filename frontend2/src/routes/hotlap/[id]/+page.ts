import { createApi } from "#lib/api.js";
import { NotFoundError } from "@lfsplanet/sdk/api";
import { error } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({
  fetch,
  params,
  parent,
  depends,
  url,
}) => {
  depends("app:hotlaps");
  depends("app:reports");
  depends("app:comments");
  const id = Number(params.id);
  if (!/^[1-9]\d*$/.test(params.id) || !Number.isSafeInteger(id))
    error(404, "Hotlap not found");
  const { breadcrumbs, eras } = await parent();
  const api = createApi(fetch);
  const hotlap = await api.hotlaps.getHotlap({ hotlap: id }).catch((cause) => {
    if (cause instanceof NotFoundError) error(404, "Hotlap not found");
    throw cause;
  });
  const around = url.searchParams.get("around");
  if (
    around &&
    (!/^[1-9]\d*$/.test(around) || !Number.isSafeInteger(Number(around)))
  )
    error(400, "Invalid comment");
  const [comments, chart] = await Promise.all([
    hotlap.state === "valid"
      ? api.comments.listHotlapComments({
          hotlap: id,
          page: Number(url.searchParams.get("comments_page") ?? 1),
          around:
            around && !url.searchParams.has("comments_page")
              ? Number(around)
              : undefined,
        })
      : null,
    // Chart identity includes the canonical catalogue names and artwork, even
    // when the leaderboard is empty. Keep the requested lap page bounded.
    api.charts
      .best({
        era: hotlap.era_id,
        track: hotlap.track,
        vehicle: hotlap.vehicle,
        per_page: 1,
      })
      .catch((cause) => {
        if (cause instanceof NotFoundError) return null;
        throw cause;
      }),
  ]);
  const era = eras.find((candidate) => candidate.id === hotlap.era_id);
  return {
    hotlap,
    track: chart?.chart.track ?? null,
    vehicle: chart?.chart.vehicle ?? null,
    worldRecord: chart?.world_record ?? null,
    comments,
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Hotlaps" },
      {
        label: hotlap.era_title,
        href: `/hotlaps/${hotlap.era_id}`,
        tag: era
          ? { label: era.open ? "Open" : "Historical", accent: era.open }
          : undefined,
      },
      { label: `Hotlap #${id}`, href: `/hotlap/${id}` },
    ],
  };
};
