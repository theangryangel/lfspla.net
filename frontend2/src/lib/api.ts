/** API response types and request helpers. */
import { error } from "@sveltejs/kit";

export type Fetch = typeof globalThis.fetch;

export interface ApiErrorBody {
  error: { code: string; message: string; details?: unknown };
}

/** GET an endpoint. */
export async function get<T>(fetch: Fetch, path: string): Promise<T> {
  const response = await fetch(path, {
    headers: { accept: "application/json" },
  });
  if (!response.ok) throw error(response.status, await message(response));
  return (await response.json()) as T;
}

/** Return null for expected absent statuses. */
export async function getOptional<T>(
  fetch: Fetch,
  path: string,
  absent: number[] = [401, 404],
): Promise<T | null> {
  const response = await fetch(path, {
    headers: { accept: "application/json" },
  });
  if (absent.includes(response.status)) return null;
  if (!response.ok) throw error(response.status, await message(response));
  return (await response.json()) as T;
}

export interface Pagination {
  page: number;
  per_page: number;
  total_items: number;
  total_pages: number;
}

export interface ListResponse<T> {
  items: T[];
}

export interface PaginatedResponse<T> extends ListResponse<T> {
  pagination: Pagination;
}

export async function getList<T>(fetch: Fetch, path: string): Promise<T[]> {
  return (await get<ListResponse<T>>(fetch, path)).items;
}

export async function getOptionalList<T>(
  fetch: Fetch,
  path: string,
  absent: number[] = [401, 404],
): Promise<T[] | null> {
  return (
    (await getOptional<ListResponse<T>>(fetch, path, absent))?.items ?? null
  );
}

/** A failed API request. Status 0 means it never reached the API. */
export class RequestFailed extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    readonly details?: unknown,
  ) {
    super(message);
    this.name = "RequestFailed";
  }

  get retryable(): boolean {
    return (
      this.status === 0 ||
      this.status === 408 ||
      this.status === 429 ||
      this.status >= 500
    );
  }
}

/** Send an authenticated API request. */
export async function send<T>(
  path: string,
  {
    method,
    csrf,
    body,
    json,
  }: { method: string; csrf: string; body?: BodyInit; json?: unknown },
): Promise<T | null> {
  let response: Response;
  try {
    response = await fetch(path, {
      method,
      headers: {
        accept: "application/json",
        "X-CSRF-Token": csrf,
        ...(json !== undefined ? { "Content-Type": "application/json" } : {}),
      },
      body: json !== undefined ? JSON.stringify(json) : body,
    });
  } catch {
    throw new RequestFailed(
      0,
      "network_error",
      "The request could not be sent.",
    );
  }
  if (!response.ok) throw await failure(response);
  return response.status === 204 ? null : ((await response.json()) as T);
}

async function failure(response: Response): Promise<RequestFailed> {
  let code = "request_failed";
  let text =
    response.statusText || `Request failed with status ${response.status}`;
  let details: unknown;
  try {
    const body = (await response.json()) as ApiErrorBody;
    if (body?.error?.code) code = body.error.code;
    if (body?.error?.message) text = body.error.message;
    details = body?.error?.details;
  } catch {
    // The response was not an API error.
  }
  return new RequestFailed(response.status, code, text, details);
}

async function message(response: Response): Promise<string> {
  return (await failure(response)).message;
}

export function query(
  params: Record<string, string | number | undefined | null>,
): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "")
      search.set(key, String(value));
  }
  const encoded = search.toString();
  return encoded ? `?${encoded}` : "";
}

export interface PlayerSummary {
  id: number;
  lfs_username: string;
  display_name: string;
  country_code: string | null;
}

export type SteeringInput =
  "wheel" | "mouse" | "keyboard" | "keyboard_stabilised";
export type DriverSide = "left" | "right";
export type HotlapState = "pending" | "valid" | "invalid";
export type PodiumLevel = "gold" | "silver" | "bronze";

export type PlayerBadge =
  | {
      kind: "ranking";
      ranking_id: string;
      label: string;
      title: string;
      ranking_position: number;
    }
  | {
      kind: "ranking_completion";
      ranking_id: string;
      label: string;
      title: string;
    }
  | {
      kind: "world_record_podium";
      level: PodiumLevel;
      firsts: number;
      seconds: number;
      thirds: number;
    }
  | {
      kind: "world_record_leader";
      leader_position: number;
      world_records: number;
    }
  | { kind: "newbie" };

export interface MeResponse {
  authenticated: boolean;
  player: PlayerSummary | null;
  csrf_token: string;
  allow_test_validation: boolean;
  allow_uploads: boolean;
  max_spr_upload_bytes: number;
}

export interface EraSummary {
  id: string;
  title: string;
  open: boolean;
  version_requirement: string;
  rankings: RankingSummary[];
}

export interface RankingSummary {
  id: string;
  title: string;
  description: string;
  my_progress: RankingProgressResponse | null;
}

export interface RankingRules {
  benchmark_percent: number;
  nation_max_points: number;
  nation_driver_limit: number;
}

export interface RankingChart {
  track: string;
  vehicle: string;
  my_hotlap: BestHotlapResponse | null;
}

export interface RankingProgressResponse {
  total_combinations: number;
  completed_combinations: number;
}

export interface RankingDetailResponse {
  id: string;
  title: string;
  description: string;
  rules: RankingRules;
  charts: RankingChart[];
  my_progress: RankingProgressResponse | null;
}

export interface PersonalRankingResponse {
  ranking_id: string;
  title: string;
  total_charts: number;
  entries: PersonalRankingEntry[];
}

export interface PersonalRankingEntry {
  position: number;
  player_id: number;
  lfs_username: string;
  display_name: string;
  country_code: string | null;
  completed_charts: number;
  total_charts: number;
  handicap_ms: number;
  badges: PlayerBadge[];
}

export interface NationRankingResponse {
  ranking_id: string;
  title: string;
  total_charts: number;
  entries: NationRankingEntry[];
}

export interface NationRankingEntry {
  position: number;
  country_code: string;
  points: number;
  handicap_ms: number;
  contributing_laps: number;
  contributing_charts: number;
}

export interface NationContribution {
  player_id: number;
  lfs_username: string;
  display_name: string;
  points: number;
  contributing_charts: number;
  handicap_ms: number;
}

export interface TrackSummary {
  code: string;
  name: string;
  location: TrackLocation;
  reverse: boolean;
  open_configuration: boolean;
}

export interface TrackLocation {
  code: TrackLocationCode;
  name: string;
}

export type TrackLocationCode =
  "BL" | "SO" | "FE" | "KY" | "AS" | "WE" | "RO" | "OTHER";

export interface VehicleSummary {
  code: string;
  name: string;
  license: string;
  image_url: string | null;
}

export interface CombinationSummary {
  track: TrackSummary;
  vehicle: VehicleSummary;
}

export type CombinationRejection =
  "unknown_track" | "open_configuration" | "unknown_vehicle" | "not_offered";

/** Combination validation returns a reason instead of an error. */
export interface CombinationCheck {
  valid: boolean;
  combination: CombinationSummary | null;
  reason: CombinationRejection | null;
}

export type Ordering = "asc" | "desc";
export type HotlapListColumn = "submitted" | "driver" | "rank" | "lap_time";
export type BestHotlapColumn = "rank" | "driver" | "set";

export interface HotlapChartResponse extends PaginatedResponse<BestHotlapResponse> {
  era_id: string;
  track: string;
  vehicle: string;
}

export interface BestHotlapResponse {
  position: number;
  id: number;
  spr_url: string | null;
  player: PlayerSummary & { badges: PlayerBadge[] };
  track: string;
  vehicle: string;
  lap_time_ms: number;
  distance_to_benchmark_ms: number;
  distance_to_world_record_ms: number;
  split_1_ms: number;
  split_2_ms: number;
  split_3_ms: number;
  split_4_ms: number;
  steering: SteeringInput;
  brake_help_enabled: boolean;
  automatic_gears: boolean;
  manual_shifter: boolean | null;
  axis_clutch: boolean;
  automatic_clutch: boolean;
  driver_side: DriverSide;
  abs_enabled: boolean | null;
  created_at: string;
  game_version: string;
}

export interface ManagedHotlapResponse {
  id: number;
  player_id: number;
  era_id: string;
  era_title: string;
  track: string;
  vehicle: string | null;
  raw_vehicle_name: string;
  mod_version: number | null;
  lap_time_ms: number;
  position?: number | null;
  distance_to_world_record_ms?: number | null;
  contributes_to?: RankingContribution[];
  split_1_ms: number;
  split_2_ms: number;
  split_3_ms: number;
  split_4_ms: number;
  original_filename: string | null;
  steering: SteeringInput;
  brake_help_enabled: boolean;
  automatic_gears: boolean;
  manual_shifter: boolean | null;
  axis_clutch: boolean;
  automatic_clutch: boolean;
  driver_side: DriverSide;
  abs_enabled: boolean | null;
  created_at: string;
  game_version: string;
  state: HotlapState;
  hlvc_result_code: number | null;
  error_detail: string | null;
  replay_url: string | null;
}

export interface RankingContribution {
  id: string;
  title: string;
}

export type PlayerSearchResponse = ListResponse<PlayerSummary>;

export interface PlayerChartResultResponse {
  hotlap_id: number;
  spr_url: string | null;
  era_id: string;
  track: string;
  vehicle: string;
  lap_time_ms: number;
  distance_to_world_record_ms: number;
  position: number;
  entries: number;
  created_at: string;
  game_version: string;
}

export interface PlayerStats {
  hotlaps: number;
  personal_bests: number;
  world_records: number;
  podiums: number;
  eras: number;
  first_hotlap_at: string | null;
  latest_hotlap_at: string | null;
}

export interface PlayerEraStats {
  id: string;
  title: string;
  hotlaps: number;
  personal_bests: number;
  firsts: number;
  seconds: number;
  thirds: number;
  first_hotlap_at: string;
  latest_hotlap_at: string;
  badges: PlayerBadge[];
}

export interface PlayerResponse {
  id: number;
  lfs_username: string;
  display_name: string;
  country_code: string | null;
  stats: PlayerStats;
  eras: PlayerEraStats[];
  highlights: PlayerChartResultResponse[];
}

export interface CountrySummary {
  code: string;
  name: string;
}

export interface HotlapActivityResponse {
  submission?: ManagedHotlapResponse;
  id: number;
  player: PlayerSummary;
  era_id: string;
  track: string;
  vehicle: string | null;
  lap_time_ms: number;
  position: number | null;
  distance_to_world_record_ms: number | null;
  contributes_to: RankingContribution[];
  state: HotlapState;
  created_at: string;
}

export interface PlayerComparisonResponse {
  left: { player: PlayerSummary; results: PlayerChartResultResponse[] };
  right: { player: PlayerSummary; results: PlayerChartResultResponse[] };
}

export interface StatsResponse {
  validated_hotlaps: number;
  drivers: number;
  combinations: number;
  eras: number;
}

export interface PersonalAccessTokenResponse {
  id: number;
  name: string;
  token_hint: string;
  created_at: string;
  expires_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
}

export interface CreatedPersonalAccessTokenResponse {
  token: string;
  credential: PersonalAccessTokenResponse;
}

export interface WorldRecordHolderResponse {
  position: number;
  player: PlayerSummary;
  world_records: number;
}

export interface WebhookResponse {
  id: number;
  name: string;
  format: "discord";
  event_kind: "hotlap_validated" | "world_record_set";
  enabled: boolean;
}

export interface WebhookNotificationResponse {
  id: number;
  webhook_name: string;
  event_kind: WebhookResponse["event_kind"];
  status: "pending" | "delivered" | "failed";
  attempt_count: number;
  created_at: string;
  next_attempt_at: string | null;
  delivered_at: string | null;
  failed_at: string | null;
  error_detail: string | null;
}

export interface WebhookOptionsResponse {
  formats: {
    value: WebhookResponse["format"];
    label: string;
    url_placeholder: string;
    url_help: string;
  }[];
  events: {
    value: WebhookResponse["event_kind"];
    label: string;
    description: string;
  }[];
}
