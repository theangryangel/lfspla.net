import { error } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const load: PageLoad = ({ url }) => {
  function positiveId(name: string): number | undefined {
    const value = url.searchParams.get(name);
    if (value === null) return undefined;
    const id = Number(value);
    if (!Number.isSafeInteger(id) || id <= 0)
      error(400, "Invalid event filter");
    return id;
  }
  const leagueId = positiveId("league_id");
  const seasonId = positiveId("season_id");
  return {
    leagueId,
    seasonId,
    filtered: leagueId !== undefined || seasonId !== undefined,
  };
};
