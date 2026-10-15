import { createApi } from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url, fetch }) => {
  const search = url.searchParams.get("q")?.trim() ?? "";
  const requestedPage = Number(url.searchParams.get("page") ?? "1");
  const page =
    Number.isSafeInteger(requestedPage) && requestedPage > 0
      ? requestedPage
      : 1;
  const vehicles = await createApi(fetch).vehicles.listVehicles({
    q: search || undefined,
    page,
    per_page: 25,
  });
  return { search, vehicles };
};
