import {
  getOptional,
  type RankingStandings,
  type NationRankingEntry,
  type PersonalRankingEntry,
} from "$lib/api.js";
import type { PageLoad } from "./$types";

/**
 * Loads both standings for one ranking.
 *
 * Either answers 409 until an operator has selected the ranking's charts, so a
 * missing standing is an ordinary state the page reports rather than an error.
 */
export const load: PageLoad = async ({ depends, params, fetch }) => {
  depends("app:hotlaps");
  const base = `/api/v1/eras/${encodeURIComponent(params.era)}/rankings/${encodeURIComponent(params.rank)}`;
  const absent = [401, 404, 409];
  const [players, nations] = await Promise.all([
    getOptional<RankingStandings<PersonalRankingEntry>>(
      fetch,
      `${base}/players`,
      absent,
    ),
    getOptional<RankingStandings<NationRankingEntry>>(
      fetch,
      `${base}/nations`,
      absent,
    ),
  ]);

  return { players, nations };
};
