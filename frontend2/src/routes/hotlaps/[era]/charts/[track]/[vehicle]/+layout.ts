import {
  get,
  query,
  type CombinationCheck,
  type CombinationRejection,
} from "$lib/api.js";
import { hotlapPath } from "$lib/era.js";
import type { LayoutLoad } from "./$types";

// Combination metadata depends on the path, not chart filters or pagination.
export const load: LayoutLoad = async ({ params, fetch, parent }) => {
  const pair = await get<CombinationCheck>(
    fetch,
    `/api/v1/eras/${encodeURIComponent(params.era)}/combinations/validate` +
      query({ track: params.track, vehicle: params.vehicle }),
  );
  const combination = pair.valid ? pair.combination : null;
  const reason: CombinationRejection | null = combination
    ? null
    : (pair.reason ?? "not_offered");
  const { breadcrumbs } = await parent();
  const label = combination
    ? `${combination.track.code} / ${combination.vehicle.code}`
    : `${params.track} / ${params.vehicle}`;
  return {
    track: combination?.track ?? null,
    vehicle: combination?.vehicle ?? null,
    reason,
    breadcrumbs: [
      ...breadcrumbs,
      {
        label,
        href: `${hotlapPath(params.era)}/charts/${encodeURIComponent(params.track)}/${encodeURIComponent(params.vehicle)}`,
      },
    ],
  };
};
