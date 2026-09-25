import { get, query, type HotlapChartResponse } from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({
  depends,
  params,
  url,
  fetch,
  parent,
}) => {
  depends("app:hotlaps");
  const { countries, track, vehicle } = await parent();
  const era = encodeURIComponent(params.era);
  const requestedCountry = url.searchParams.get("country");
  const country = requestedCountry ? match(countries, requestedCountry) : null;

  const chart =
    track && vehicle
      ? await get<HotlapChartResponse>(
          fetch,
          `/api/v1/eras/${era}/charts/${encodeURIComponent(track.code)}/${encodeURIComponent(vehicle.code)}` +
            query({
              country: requestedCountry,
              controller: url.searchParams.get("controller"),
              page: url.searchParams.get("page"),
              per_page: url.searchParams.get("per_page"),
              column: url.searchParams.get("column"),
              order: url.searchParams.get("order"),
            }),
        )
      : null;

  return { country, chart };
};

/** The catalogue entry a code names, ignoring case as the API does. */
function match<T extends { code: string }>(
  options: T[],
  code: string,
): T | null {
  const wanted = code.toLowerCase();
  return options.find((option) => option.code.toLowerCase() === wanted) ?? null;
}
