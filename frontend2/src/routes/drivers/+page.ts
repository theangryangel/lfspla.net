import { error } from "@sveltejs/kit";
import { LfsplanetApiError } from "@lfsplanet/sdk";
import { createApi, apiErrorMessage } from "#lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url, fetch }) => {
  const api = createApi(fetch);
  const filters = {
    q: url.searchParams.get("q")?.trim() ?? "",
    country: url.searchParams.get("country")?.trim().toUpperCase() ?? "",
  };
  const requestedPage = Number(url.searchParams.get("page") ?? "1");
  const page =
    Number.isSafeInteger(requestedPage) && requestedPage > 0
      ? requestedPage
      : 1;
  try {
    const [players, countries] = await Promise.all([
      api.players.search({
        q: filters.q || undefined,
        country: filters.country || undefined,
        page,
        per_page: 25,
      }),
      api.countries.listCountries(),
    ]);
    return { filters, players, countries: countries.items };
  } catch (cause) {
    error(
      cause instanceof LfsplanetApiError && cause.statusCode === 400
        ? 400
        : 502,
      apiErrorMessage(cause),
    );
  }
};
