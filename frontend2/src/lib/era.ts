import type { EraSummary } from "$lib/api.js";

/**
 * The era that `/hotlaps/current` and every "current era" link resolve to.
 *
 * `GET /api/v1/eras` returns them oldest first, so the newest era still open
 * for submissions wins; an all-archived catalogue falls back to the newest.
 */
export function currentEra(eras: EraSummary[]): EraSummary | null {
  return [...eras].reverse().find((era) => era.open) ?? eras.at(-1) ?? null;
}

/** Route prefix for one era's hotlap pages. */
export function hotlapPath(eraId: string): string {
  return `/hotlaps/${eraId}`;
}
