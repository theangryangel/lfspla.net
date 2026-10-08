import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, parent }) => {
  const api = createApi(fetch);
  const { me, breadcrumbs } = await parent();
  return {
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Personal access tokens", href: "/account/tokens" },
    ],
    tokens: me.authenticated
      ? await api.personalAccessTokens
          .listPersonalAccessTokens()
          .then((response) => response.items)
      : [],
  };
};
