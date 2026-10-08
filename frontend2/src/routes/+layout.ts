import { createApi } from "$lib/api.js";

import type { Crumb } from "$lib/breadcrumbs.js";
import type { LayoutLoad } from "./$types";

// Single-page app: no server rendering, no prerendering. The Rust backend serves
// dist/ and falls back to index.html for deep frontend routes.
export const ssr = false;
export const prerender = false;

/**
 * Loads the two things every page needs: who is signed in, and which eras exist.
 *
 * Both are cheap and change rarely, and having them here means `invalidateAll()`
 * after a sign-out refreshes the whole tree in one pass.
 */
export const load: LayoutLoad = async ({ fetch }) => {
  const api = createApi(fetch);
  const [me, eras] = await Promise.all([
    api.authentication.get(),
    api.eras.listEras().then((response) => response.items),
  ]);

  const breadcrumbs: Crumb[] = [];
  return { me, eras, breadcrumbs };
};
