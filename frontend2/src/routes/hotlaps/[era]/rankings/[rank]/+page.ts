import {
  UnauthorizedError,
  NotFoundError,
  ConflictError,
} from "@lfsplanet/sdk/api";

import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

/**
 * Loads both standings for one ranking.
 *
 * Either answers 409 until an operator has selected the ranking's charts, so a
 * missing standing is an ordinary state the page reports rather than an error.
 */
export const load: PageLoad = async ({ depends, params, fetch }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const [players, nations] = await Promise.all([
    api.ranking
      .players({ era: params.era, ranking: params.rank })
      .catch((cause) => {
        if (
          cause instanceof UnauthorizedError ||
          cause instanceof NotFoundError ||
          cause instanceof ConflictError
        )
          return null;
        throw cause;
      }),
    api.ranking
      .nations({ era: params.era, ranking: params.rank })
      .catch((cause) => {
        if (
          cause instanceof UnauthorizedError ||
          cause instanceof NotFoundError ||
          cause instanceof ConflictError
        )
          return null;
        throw cause;
      }),
  ]);

  return { players, nations };
};
