import { UnauthorizedError, type ListHotlapsRequest } from "@lfsplanet/sdk/api";
import { createApi } from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({
  depends,
  fetch,
  parent,
  url,
  params,
}) => {
  depends("app:hotlaps");
  const { me, breadcrumbs } = await parent();
  const trail = [...breadcrumbs, { label: "My submissions" }];
  if (!me.authenticated) return { hotlaps: null, breadcrumbs: trail };
  const api = createApi(fetch);
  const page = url.searchParams.get("page");
  const hotlaps = await api.hotlaps
    .listHotlaps({
      mine: true,
      era_id: params.era,
      page: page ? Number(page) : undefined,
      column: (url.searchParams.get("column") ||
        undefined) as ListHotlapsRequest["column"],
      order: (url.searchParams.get("order") ||
        undefined) as ListHotlapsRequest["order"],
    })
    .catch((cause) => {
      if (cause instanceof UnauthorizedError) return null;
      throw cause;
    });
  if (hotlaps?.items.some((lap) => !lap.submission))
    throw new Error("Upload details are missing from the response.");
  return { hotlaps, breadcrumbs: trail };
};
