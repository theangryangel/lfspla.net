import { error } from "@sveltejs/kit";
import { apiErrorBody, createApi } from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params, url, fetch, parent }) => {
  const api = createApi(fetch);
  const location = await api.trackLocations
    .getTrackLocation({ code: params.code })
    .catch((cause: unknown) => {
      if (apiErrorBody(cause)?.code === "track_location_not_found") {
        error(404, "Track location not found");
      }
      throw cause;
    });
  const search = url.searchParams.get("q")?.trim() ?? "";
  const requestedPage = Number(url.searchParams.get("page") ?? "1");
  const page =
    Number.isSafeInteger(requestedPage) && requestedPage > 0
      ? requestedPage
      : 1;
  const [tracks, { breadcrumbs }] = await Promise.all([
    api.tracks.listTracks({
      location: location.code,
      q: search || undefined,
      page,
      per_page: 25,
    }),
    parent(),
  ]);
  return {
    location,
    search,
    tracks,
    breadcrumbs: [
      ...breadcrumbs,
      {
        label: location.name,
        href: `/tracks/${encodeURIComponent(location.code)}`,
      },
    ],
  };
};
