import { getList, type PersonalAccessTokenResponse } from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, parent }) => {
  const { me, breadcrumbs } = await parent();
  return {
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Personal access tokens", href: "/account/tokens" },
    ],
    tokens: me.authenticated
      ? await getList<PersonalAccessTokenResponse>(
          fetch,
          "/api/v1/me/personal-access-tokens",
        )
      : [],
  };
};
