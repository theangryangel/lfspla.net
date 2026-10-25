import { error } from "@sveltejs/kit";
import { LfsplanetApiError } from "@lfsplanet/sdk";
import { createApi, apiErrorMessage } from "#lib/api.js";
import { filterKeys } from "#lib/vehicle-explorer.js";
import type { ListVehiclesRequest } from "@lfsplanet/sdk/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url, fetch }) => {
  const filters = Object.fromEntries(
    filterKeys.map((key) => [key, url.searchParams.get(key)?.trim() ?? ""]),
  );
  const requestedPage = Number(url.searchParams.get("page") ?? "1");
  const page =
    Number.isSafeInteger(requestedPage) && requestedPage > 0
      ? requestedPage
      : 1;
  const request: Record<string, string | number | boolean> = {
    page,
    per_page: 24,
  };
  for (const [key, value] of Object.entries(filters)) {
    if (!value) continue;
    request[key] =
      key.startsWith("min_") || key.startsWith("max_") ? Number(value) : value;
  }
  // Keep URL values intact for validation by the API's query extractor.
  for (const key of ["approved", "electric"]) {
    if (filters[key] === "true" || filters[key] === "false")
      request[key] = filters[key] === "true";
    else if (filters[key]) error(400, `Invalid ${key} filter`);
  }
  try {
    const vehicles = await createApi(fetch).vehicles.listVehicles(
      request as ListVehiclesRequest,
    );
    return { search: filters.q, filters, vehicles };
  } catch (cause) {
    error(
      cause instanceof LfsplanetApiError && cause.statusCode === 400
        ? 400
        : 502,
      apiErrorMessage(cause),
    );
  }
};
