import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

/** Preserve directory bookmarks as an entry to the era's ranking picker. */
export const load: PageLoad = ({ params, url }) => {
  const query = new URLSearchParams({ picker: "rankings" });
  const search = url.searchParams.get("q");
  if (search) query.set("q", search);
  redirect(308, `/hotlaps/${encodeURIComponent(params.era)}?${query}`);
};
