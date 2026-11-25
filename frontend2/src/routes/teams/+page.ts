import { error, redirect } from "@sveltejs/kit";
import { LfsplanetApiError } from "@lfsplanet/sdk";
import { createApi, apiErrorMessage } from "#lib/api.js";
import type { Tag } from "#lib/api.js";
import { disciplineLabels } from "#lib/teams.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url, fetch, parent }) => {
  const mine = url.searchParams.get("mine") ?? "";
  if (!["", "true", "false"].includes(mine))
    error(400, "Choose a valid membership filter.");
  const { me } = await parent();
  if (mine === "true" && !me.authenticated) {
    const target = new URL(url);
    target.searchParams.delete("mine");
    target.searchParams.delete("page");
    redirect(307, target.pathname + target.search);
  }
  const filters = {
    q: url.searchParams.get("q")?.trim() ?? "",
    discipline: url.searchParams.get("discipline") ?? "",
    accepts_applications: url.searchParams.get("accepts_applications") ?? "",
    mine: mine === "true" ? "true" : "",
  };
  if (
    filters.discipline &&
    !Object.hasOwn(disciplineLabels, filters.discipline)
  )
    error(400, "Choose a valid team discipline.");
  if (!["", "true", "false"].includes(filters.accepts_applications))
    error(400, "Choose a valid application status.");
  const requestedPage = Number(url.searchParams.get("page") ?? "1");
  const page =
    Number.isSafeInteger(requestedPage) && requestedPage > 0
      ? requestedPage
      : 1;
  try {
    const teams = await createApi(fetch).teams.listTeams({
      q: filters.q || undefined,
      mine: filters.mine ? true : undefined,
      discipline: (filters.discipline || undefined) as Tag | undefined,
      accepts_applications: filters.accepts_applications
        ? filters.accepts_applications === "true"
        : undefined,
      page,
      per_page: 25,
    });
    return { teams, filters };
  } catch (cause) {
    error(
      cause instanceof LfsplanetApiError &&
        (cause.statusCode === 400 || cause.statusCode === 401)
        ? cause.statusCode
        : 502,
      apiErrorMessage(cause),
    );
  }
};
