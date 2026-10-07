import {
  getOptional,
  query,
  type Fetch,
  type Hotlap,
  type PaginatedResponse,
} from "$lib/api.js";

/** One page of private upload details from the shared collection endpoint. */
export async function getMyHotlaps(
  fetch: Fetch,
  params: Record<string, string | number | null | undefined> = {},
): Promise<PaginatedResponse<Hotlap> | null> {
  const response = await getOptional<PaginatedResponse<Hotlap>>(
    fetch,
    "/api/v1/hotlaps" + query({ ...params, mine: "true" }),
    [401],
  );
  if (!response) return null;
  if (response.items.some((lap) => !lap.submission))
    throw new Error("Upload details are missing from the response.");
  return response;
}

export function submissionQuery(url: URL) {
  return {
    page: url.searchParams.get("page"),
    column: url.searchParams.get("column"),
    order: url.searchParams.get("order"),
  };
}
