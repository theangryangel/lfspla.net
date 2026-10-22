import { error } from "@sveltejs/kit";
import { createApi } from "#lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url, fetch }) => {
  function positiveId(name: string): number | undefined {
    const value = url.searchParams.get(name);
    if (value === null) return undefined;
    const id = Number(value);
    if (!Number.isSafeInteger(id) || id <= 0)
      error(400, "Invalid event filter");
    return id;
  }
  const requestedPage = Number(url.searchParams.get("page") ?? "1");
  const page =
    Number.isSafeInteger(requestedPage) && requestedPage > 0
      ? requestedPage
      : 1;
  const leagueId = positiveId("league_id");
  const seasonId = positiveId("season_id");
  const events = await createApi(fetch).events.listEvents({
    league_id: leagueId,
    season_id: seasonId,
    page,
    per_page: 25,
  });
  return { events, filtered: leagueId !== undefined || seasonId !== undefined };
};
