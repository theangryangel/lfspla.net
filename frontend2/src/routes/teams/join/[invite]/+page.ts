import { error } from "@sveltejs/kit";
import { LfsplanetApiError } from "@lfsplanet/sdk";
import { createApi, apiErrorMessage } from "#lib/api.js";
import type { PageLoad } from "./$types";
export const load: PageLoad = async ({ params, fetch, parent }) => {
  try {
    const preview = await createApi(fetch).teams.previewTeamInvite({
      token: params.invite,
    });
    const { breadcrumbs } = await parent();
    return {
      ...preview,
      token: params.invite,
      title: `Join ${preview.team.name}`,
      breadcrumbs: [...breadcrumbs, { label: "Team invitation" }],
    };
  } catch (cause) {
    if (cause instanceof LfsplanetApiError && cause.statusCode === 404)
      error(
        404,
        "This invite link has expired, been revoked or reached its usage limit. Ask the team owner for a new link.",
      );
    error(502, apiErrorMessage(cause));
  }
};
