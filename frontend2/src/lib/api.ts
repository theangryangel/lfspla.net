import { progressFetch } from "#lib/loading-progress.js";
import { getContext, setContext } from "svelte";
import { LfsplanetApiClient, LfsplanetApiError } from "@lfsplanet/sdk";
import type { ErrorBody, ErrorResponse } from "@lfsplanet/sdk/types";
export type * from "@lfsplanet/sdk/types";
export type {
  NationContributionResponse as NationContribution,
  NationRankingEntryResponse as NationRankingEntry,
  PersonalRankingEntryResponse as PersonalRankingEntry,
  TrackLocationSummary as TrackLocation,
} from "@lfsplanet/sdk/types";
export type {
  ListHotlapsRequestColumn as HotlapListColumn,
  ListHotlapsRequestOrder as Ordering,
} from "@lfsplanet/sdk/api";

export type Fetch = typeof globalThis.fetch;
export type ApiClient = LfsplanetApiClient;

/** Load functions supply their fetch to preserve SvelteKit invalidation. */
export function createApi(
  fetch?: Fetch,
  { progress = !fetch }: { progress?: boolean } = {},
): ApiClient {
  return new LfsplanetApiClient({
    environment: "/",
    auth: false,
    maxRetries: 0,
    fetch:
      typeof window !== "undefined" && progress
        ? progressFetch(fetch ?? globalThis.fetch)
        : fetch,
  });
}

const API_CONTEXT = Symbol("lfsplanet.api");

export function setApi(api: ApiClient = createApi()): ApiClient {
  return setContext(API_CONTEXT, api);
}

export function useApi(): ApiClient {
  return getContext<ApiClient>(API_CONTEXT);
}

export function apiErrorBody(cause: unknown): ErrorBody | undefined {
  if (!(cause instanceof LfsplanetApiError)) return undefined;
  return (cause.body as Partial<ErrorResponse> | undefined)?.error;
}

/** The first field message from a `validation_error`, shaped `{ field: [{ message }] }`. */
function validationMessage(body: ErrorBody | undefined): string | undefined {
  if (body?.code !== "validation_error") return undefined;
  const fields = body.details;
  if (!fields || typeof fields !== "object") return undefined;
  for (const errors of Object.values(fields as Record<string, unknown>)) {
    if (!Array.isArray(errors)) continue;
    for (const error of errors) {
      const message = (error as { message?: unknown } | null)?.message;
      if (typeof message === "string" && message) return message;
    }
  }
  return undefined;
}

/** Show the backend's message rather than Fern's status-class name. */
export function apiErrorMessage(cause: unknown): string {
  const body = apiErrorBody(cause);
  return (
    validationMessage(body) ??
    body?.message ??
    (cause instanceof LfsplanetApiError && cause.statusCode == null
      ? "The request could not be sent."
      : cause instanceof Error
        ? cause.message
        : "The request failed.")
  );
}

/** The browser client disables automatic retries; the upload UI decides when to offer one. */
export function apiErrorRetryable(cause: unknown): boolean {
  if (!(cause instanceof LfsplanetApiError)) return false;
  const status = cause.statusCode;
  return status == null || status === 408 || status === 429 || status >= 500;
}
