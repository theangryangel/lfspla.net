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
  const { breadcrumbs } = await parent();
  const api = createApi(fetch);
  const hotlap = await api.hotlaps.getHotlap({ hotlap: id }).catch((cause) => {
    if (cause instanceof NotFoundError) error(404, "Hotlap not found");
    throw cause;
  });
  const around =
    url.searchParams.get("around") ?? url.searchParams.get("comment");
  if (
    around &&
    (!/^[1-9]\d*$/.test(around) || !Number.isSafeInteger(Number(around)))
  )
    error(400, "Invalid comment");
  const [summaries, comments] = await Promise.all([
    api.reports.getReportSummaries({ hotlap_ids: String(id) }),
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
  ]);
  const report = summaries.items[0] ?? null;
  return {
    hotlap,
    report,
    comments,
    breadcrumbs: [
      ...breadcrumbs,
      { label: hotlap.era_title, href: `/hotlaps/${hotlap.era_id}` },
      {
        label: `${hotlap.track} / ${hotlap.vehicle}`,
        href: `/hotlaps/${hotlap.era_id}/charts/${encodeURIComponent(hotlap.track)}/${encodeURIComponent(hotlap.vehicle)}`,
      },
      { label: `Hotlap #${id}`, href: `/hotlap/${id}` },
    ],
  };
};
