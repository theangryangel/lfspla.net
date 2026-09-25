import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

/**
 * The chart itself lives at `/charts/{track}/{vehicle}`. The old index URL is
 * retained as a bookmark-compatible route to the picker on the era overview.
 */
export const load: PageLoad = ({ params, url }) => {
  // A chart used to be named by the query string. Saved links keep working,
  // permanently redirected to the path that names the same combination.
  const track = url.searchParams.get("track");
  const vehicle = url.searchParams.get("vehicle");
  if (track && vehicle) {
    const filters = new URLSearchParams(url.searchParams);
    filters.delete("track");
    filters.delete("vehicle");
    const search = filters.toString();
    redirect(
      308,
      `/hotlaps/${encodeURIComponent(params.era)}/charts/${encodeURIComponent(track)}/${encodeURIComponent(vehicle)}` +
        (search ? `?${search}` : ""),
    );
  }

  redirect(308, `/hotlaps/${encodeURIComponent(params.era)}?picker=charts`);
};
