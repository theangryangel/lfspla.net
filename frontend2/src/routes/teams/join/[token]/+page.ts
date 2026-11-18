import { createTeamsApi, TeamApiError, teamErrorMessage } from "#lib/teams.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, params }) => {
  try {
    const team = await createTeamsApi(fetch).preview(params.token);
    return {
      team,
      token: params.token,
      invalid: false,
      loadError: "",
      title: `Join ${team.name}`,
    };
  } catch (cause) {
    const invalid =
      cause instanceof TeamApiError && [400, 404, 410].includes(cause.status);
    return {
      team: null,
      token: params.token,
      invalid,
      loadError: invalid
        ? "This invite link is invalid or has been disabled. Ask a team owner or admin for a new link."
        : teamErrorMessage(cause),
      title: "Team invitation",
    };
  }
};
