import { getList, query, type TrackSummary } from "$lib/api.js";

const requests = new Map<string, Promise<TrackSummary[]>>();

/**
 * An era's tracks, or only those paired with one vehicle.
 *
 * Each list is fetched once and shared, since a picker reopens on the same one
 * far more often than the catalogue changes. A resolved promise is the cache; a
 * failed one drops out so a retry really retries.
 */
export function getEraTracks(
  era: string,
  vehicle?: string | null,
): Promise<TrackSummary[]> {
  // The request is its own identity, and its encoding already keeps an era and
  // a vehicle from running into one another.
  const url =
    `/api/v1/eras/${encodeURIComponent(era)}/tracks` +
    (vehicle ? query({ vehicle }) : "");
  let request = requests.get(url);
  if (!request) {
    request = getList<TrackSummary>(fetch, url).catch((error) => {
      requests.delete(url);
      throw error;
    });
    requests.set(url, request);
  }
  return request;
}
