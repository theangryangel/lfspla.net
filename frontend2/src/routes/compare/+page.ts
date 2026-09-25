import { get, query, type PlayerComparisonResponse } from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch, url, parent }) => {
  depends("app:hotlaps");
  const { currentEraId } = await parent();
  const left = url.searchParams.get("left")?.trim() ?? "";
  const right = url.searchParams.get("right")?.trim() ?? "";
  const era = url.searchParams.get("era") ?? currentEraId ?? "";
  const track = url.searchParams.get("track")?.trim() ?? "";
  const vehicle = url.searchParams.get("vehicle")?.trim() ?? "";
  let comparison: PlayerComparisonResponse | null = null;
  let problem = "";
  if (left && right && era) {
    if (left.toLowerCase() === right.toLowerCase())
      problem = "Choose two different drivers.";
    else {
      try {
        comparison = await get<PlayerComparisonResponse>(
          fetch,
          "/api/v1/compare" + query({ left, right, era, track, vehicle }),
        );
      } catch (error) {
        problem =
          error && typeof error === "object" && "body" in error
            ? (error.body as { message: string }).message
            : "Could not load the comparison. Please try again.";
      }
    }
  }
  return { left, right, era, track, vehicle, comparison, problem };
};
