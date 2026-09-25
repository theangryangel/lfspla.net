import {
  getOptional,
  query,
  type Fetch,
  type HotlapActivityResponse,
  type ManagedHotlapResponse,
  type PaginatedResponse,
} from "$lib/api.js";

/** One page of private upload details from the shared collection endpoint. */
export async function getMyHotlaps(
  fetch: Fetch,
  params: Record<string, string | number | null | undefined> = {},
): Promise<PaginatedResponse<ManagedHotlapResponse> | null> {
  const response = await getOptional<PaginatedResponse<HotlapActivityResponse>>(
    fetch,
    "/api/v1/hotlaps" + query({ ...params, mine: "true" }),
    [401],
  );
  if (!response) return null;
  return {
    ...response,
    items: response.items.map((lap) => {
      if (!lap.submission)
        throw new Error("Upload details are missing from the response.");
      return {
        ...lap.submission,
        position: lap.position,
        distance_to_world_record_ms: lap.distance_to_world_record_ms,
        contributes_to: lap.contributes_to,
      };
    }),
  };
}

/** Ranking completion needs every matching upload, not just the first page. */
export async function getAllMyHotlaps(
  fetch: Fetch,
  era: string,
): Promise<ManagedHotlapResponse[] | null> {
  const items: ManagedHotlapResponse[] = [];
  for (let page = 1; ; page++) {
    const response = await getMyHotlaps(fetch, {
      era_id: era,
      state: "valid",
      page,
      per_page: 100,
    });
    if (!response) return null;
    items.push(...response.items);
    if (page >= response.pagination.total_pages) return items;
  }
}

export function submissionQuery(url: URL) {
  return {
    page: url.searchParams.get("page"),
    column: url.searchParams.get("column"),
    order: url.searchParams.get("order"),
  };
}
