import { error } from "@sveltejs/kit";
import { LfsplanetApiError } from "@lfsplanet/sdk";
import { apiErrorMessage, createApi } from "#lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params, fetch, parent }) => {
  const id = Number(params.team);
  if (!Number.isSafeInteger(id) || id <= 0) error(404, "Team not found");
  try {
    const detail = await createApi(fetch).teams.getTeam({ id });
    const { breadcrumbs } = await parent();
    return {
      ...detail,
      breadcrumbs: [
        ...breadcrumbs,
        { label: detail.team.name, href: `/teams/${id}` },
      ],
    };
  } catch (cause) {
    error(
      cause instanceof LfsplanetApiError && cause.statusCode === 404
        ? 404
        : 502,
      apiErrorMessage(cause),
    );
  }
};
