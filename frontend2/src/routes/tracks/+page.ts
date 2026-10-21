import { createApi } from "#lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const locations = await createApi(fetch).trackLocations.listTrackLocations();
  return { locations: locations.items };
};
