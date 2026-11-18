import { error } from "@sveltejs/kit";
import { createTeamsApi, TeamApiError, teamErrorMessage } from "#lib/teams.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, url }) => {
  const api = createTeamsApi(fetch);
  const filters = {
    q: url.searchParams.get("q")?.trim() ?? "",
    status:
      url.searchParams.get("status")?.trim() ??
      (url.searchParams.get("include_archived") === "true" ? "all" : ""),
  };
  const requestedPage = Number(url.searchParams.get("page") ?? "1");
  const page =
    Number.isSafeInteger(requestedPage) && requestedPage > 0
      ? requestedPage
      : 1;
  try {
    const teams = await api.list({
      ...filters,
      page,
      per_page: 24,
    });
    return { teams, filters, title: "Teams" };
  } catch (cause) {
    error(
      cause instanceof TeamApiError && cause.status === 400 ? 400 : 502,
      teamErrorMessage(cause),
    );
  }
};
