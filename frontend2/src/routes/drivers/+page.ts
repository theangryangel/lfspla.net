import { getList, query, type PlayerSummary } from "$lib/api.js";
import type { PageLoad } from "./$types";

/**
 * Searches the public driver directory.
 *
 * The endpoint requires search text and has no "list everyone" mode, so an
 * empty box loads nothing rather than guessing at a query.
 */
export const load: PageLoad = async ({ url, fetch }) => {
  const search = url.searchParams.get("q")?.trim() ?? "";
  if (!search) return { search, players: null };

  const players = await getList<PlayerSummary>(
    fetch,
    `/api/v1/players${query({ q: search })}`,
  );
  return { search, players };
};
