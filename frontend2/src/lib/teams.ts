import type { Fetch } from "#lib/api.js";
import type { Pagination, PlayerSummary } from "#lib/api.js";

export interface Team {
  id: number;
  name: string;
  description: string | null;
  is_active: boolean;
  website_url: string | null;
  discord_url: string | null;
  invite_enabled: boolean;
}

export interface CreateTeam {
  name: string;
  description?: string;
  website_url?: string;
  discord_url?: string;
}

export interface UpdateTeam extends Omit<
  CreateTeam,
  "description" | "website_url" | "discord_url"
> {
  description: string | null;
  website_url: string | null;
  discord_url: string | null;
}

export interface TeamMembership {
  role: "member" | "admin" | null;
}

export interface TeamMember {
  player: PlayerSummary;
  role: "member" | "admin";
}

export interface TeamDirectoryItem extends Team {
  members: TeamMember[];
}

export interface TeamWithMembers extends Team {
  members: TeamMember[];
}

export interface TeamDirectory {
  items: TeamDirectoryItem[];
  pagination: Pagination;
}

export interface TeamFilters {
  q?: string;
  status?: string;
  page?: number;
  per_page?: number;
}

export class TeamApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export function teamErrorMessage(cause: unknown): string {
  return cause instanceof TeamApiError
    ? cause.message
    : "The request could not be completed. Check your connection and try again.";
}

/** Temporary adapter until the team endpoints are included in the generated SDK. No retries. */
export function createTeamsApi(fetch: Fetch = globalThis.fetch) {
  async function request<T>(
    path: string,
    method = "GET",
    csrf?: string | null,
    body?: CreateTeam | UpdateTeam | { role: TeamMember["role"] },
  ): Promise<T> {
    if (method !== "GET" && !csrf) {
      throw new TeamApiError(
        403,
        "Your session needs refreshing. Reload the page before trying again.",
      );
    }
    const response = await fetch(`/api/v1/${path}`, {
      method,
      credentials: "same-origin",
      cache: "no-store",
      headers: {
        ...(csrf ? { "X-CSRF-Token": csrf } : {}),
        ...(body ? { "Content-Type": "application/json" } : {}),
      },
      ...(body ? { body: JSON.stringify(body) } : {}),
    });
    if (!response.ok) {
      const body = await response.json().catch(() => null);
      throw new TeamApiError(
        response.status,
        typeof body?.error?.message === "string"
          ? body.error.message
          : response.status === 401
            ? "Please sign in to continue."
            : response.status === 403
              ? "You do not have permission, or your session needs refreshing."
              : "The request failed. Please try again.",
      );
    }
    return response.status === 204
      ? (undefined as T)
      : ((await response.json()) as T);
  }
  const teamPath = (team: string) => `teams/${encodeURIComponent(team)}`;
  const invitePath = (token: string) =>
    `team-invites/${encodeURIComponent(token)}`;
  return {
    changeRole: (
      team: number,
      player: number,
      role: TeamMember["role"],
      csrf: string,
    ) =>
      request<void>(`teams/${team}/members/${player}`, "PATCH", csrf, { role }),
    removeMember: (team: number, player: number, csrf: string) =>
      request<void>(`teams/${team}/members/${player}`, "DELETE", csrf),
    list: (filters: TeamFilters = {}) => {
      const query = new URLSearchParams();
      for (const [key, value] of Object.entries(filters)) {
        if (value !== undefined && value !== "") query.set(key, String(value));
      }
      return request<TeamDirectory>(`teams?${query}`);
    },
    create: (body: CreateTeam, csrf: string | null | undefined) =>
      request<Team>("teams", "POST", csrf, body),
    get: (team: string) => request<Team>(teamPath(team)),
    members: (team: string) =>
      request<{ items: TeamMember[] }>(`${teamPath(team)}/members`),
    update: (team: string, body: UpdateTeam, csrf: string | null | undefined) =>
      request<Team>(teamPath(team), "PATCH", csrf, body),
    archive: (team: string, csrf: string | null | undefined) =>
      request<Team>(`${teamPath(team)}/archive`, "POST", csrf),
    restore: (team: string, csrf: string | null | undefined) =>
      request<Team>(`${teamPath(team)}/restore`, "POST", csrf),
    membership: (team: string) =>
      request<TeamMembership>(`${teamPath(team)}/membership`),
    regenerate: (team: string, csrf: string | null | undefined) =>
      request<{ token: string }>(`${teamPath(team)}/invite-link`, "POST", csrf),
    disable: (team: string, csrf: string | null | undefined) =>
      request<void>(`${teamPath(team)}/invite-link`, "DELETE", csrf),
    preview: (token: string) => request<Team>(invitePath(token)),
    join: (token: string, csrf: string | null | undefined) =>
      request<void>(invitePath(token), "POST", csrf),
  };
}

/** Only allow public web links from team metadata. */
export function teamWebsite(value: string | null): string | undefined {
  if (!value) return;
  try {
    const url = new URL(value);
    if (url.protocol === "https:" || url.protocol === "http:") return url.href;
  } catch {
    /* Invalid metadata is not rendered as a link. */
  }
}
