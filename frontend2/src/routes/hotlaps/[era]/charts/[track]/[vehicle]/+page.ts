import { NotFoundError } from "@lfsplanet/sdk/api";

import { type CombinationRejection, createApi } from "$lib/api.js";
import type { BestRequest } from "@lfsplanet/sdk/api";
import { error } from "@sveltejs/kit";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({
  depends,
  params,
  url,
  fetch,
  parent,
}) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const { countries, breadcrumbs } = await parent();
  const requestedCountry = url.searchParams.get("country");
  const country = requestedCountry ? match(countries, requestedCountry) : null;

  // The leaderboard supplies catalogue metadata even when its page is empty.
  const chart = await api.charts
    .best({
      era: params.era,
      track: params.track,
      vehicle: params.vehicle,
      country: requestedCountry || undefined,
      controller: url.searchParams.get("controller") || undefined,
      page: url.searchParams.get("page")
        ? Number(url.searchParams.get("page"))
        : undefined,
      per_page: url.searchParams.get("per_page")
        ? Number(url.searchParams.get("per_page"))
        : undefined,
      column: (url.searchParams.get("column") ||
        undefined) as BestRequest["column"],
      order: (url.searchParams.get("order") ||
        undefined) as BestRequest["order"],
    })
    .catch((cause) => {
      if (cause instanceof NotFoundError) return null;
      throw cause;
    });
  let reason: CombinationRejection | null = null;
  if (!chart) {
    // Resolve the rejection reason only after the chart lookup misses.
    const check = await api.eras.validateEraCombination({
      era: params.era,
      track: params.track,
      vehicle: params.vehicle,
    });
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
        href: `/hotlaps/${params.era}/charts/${encodeURIComponent(params.track)}/${encodeURIComponent(params.vehicle)}`,
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
