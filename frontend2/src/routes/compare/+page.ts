import {
  apiErrorMessage,
  type PlayerComparisonResponse,
  createApi,
} from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch, url }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const left = url.searchParams.get("left")?.trim() ?? "";
  const right = url.searchParams.get("right")?.trim() ?? "";
  const era = url.searchParams.get("era")?.trim() ?? "";
  const track = url.searchParams.get("track")?.trim() ?? "";
  const vehicle = url.searchParams.get("vehicle")?.trim() ?? "";
  const sharedOnly = url.searchParams.get("shared") !== "0";
  let comparison: PlayerComparisonResponse | null = null;
  let problem = "";
  if (left && right && era) {
    if (left.toLowerCase() === right.toLowerCase())
      problem = "Choose two different drivers.";
    else {
      try {
        comparison = await api.players.compare({
          left,
          right,
          era,
          track: track || undefined,
          vehicle: vehicle || undefined,
        });
      } catch (cause) {
        problem = apiErrorMessage(cause);
      }
    }
  }
  return {
    left,
    right,
    era,
    track,
    vehicle,
    sharedOnly,
    comparison,
    problem,
  };
};
