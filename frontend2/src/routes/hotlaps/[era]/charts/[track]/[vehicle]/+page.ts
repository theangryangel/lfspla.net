import { error } from "@sveltejs/kit";
import {
  get,
  getOptional,
  query,
  type CombinationCheck,
  type CombinationRejection,
  type HotlapChartResponse,
} from "$lib/api.js";
import { hotlapPath } from "$lib/era.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({
  depends,
  params,
  url,
  fetch,
  parent,
}) => {
  depends("app:hotlaps");
  const { countries, breadcrumbs } = await parent();
  const era = encodeURIComponent(params.era);
  const requestedCountry = url.searchParams.get("country");
  const country = requestedCountry ? match(countries, requestedCountry) : null;

  // The leaderboard supplies catalogue metadata even when its page is empty.
  const chart = await getOptional<HotlapChartResponse>(
    fetch,
    `/api/v1/eras/${era}/charts/${encodeURIComponent(params.track)}/${encodeURIComponent(params.vehicle)}` +
      query({
        country: requestedCountry,
        controller: url.searchParams.get("controller"),
        page: url.searchParams.get("page"),
        per_page: url.searchParams.get("per_page"),
        column: url.searchParams.get("column"),
        order: url.searchParams.get("order"),
      }),
    [404],
  );
  let reason: CombinationRejection | null = null;
  if (!chart) {
    // Resolve the rejection reason only after the chart lookup misses.
    const check = await get<CombinationCheck>(
      fetch,
      `/api/v1/eras/${era}/combinations/validate` +
        query({ track: params.track, vehicle: params.vehicle }),
    );
    if (check.valid) error(404, "Chart not found");
    reason = check.reason ?? "not_offered";
  }
  const track = chart?.chart.track ?? null;
  const vehicle = chart?.chart.vehicle ?? null;

  return {
    country,
    chart,
    track,
    vehicle,
    reason,
    breadcrumbs: [
      ...breadcrumbs,
      {
        label: `${track?.code ?? params.track} / ${vehicle?.code ?? params.vehicle}`,
        href: `${hotlapPath(params.era)}/charts/${encodeURIComponent(params.track)}/${encodeURIComponent(params.vehicle)}`,
      },
    ],
  };
};

/** The catalogue entry a code names, ignoring case as the API does. */
function match<T extends { code: string }>(
  options: T[],
  code: string,
): T | null {
  const wanted = code.toLowerCase();
  return options.find((option) => option.code.toLowerCase() === wanted) ?? null;
}
