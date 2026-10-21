import {
  apiErrorMessage,
  type ApiClient,
  type MeResponse,
  type PlayerSummary,
} from "#lib/api.js";
import { getContext, setContext } from "svelte";
import { refreshAll } from "$app/navigation";

const KEY = Symbol("lfsplanet.session");

/**
 * Current player and frontend policy reported by `GET /api/v1/me`.
 *
 * The root layout supplies initial state; background refreshes update policy.
 * Signing in leaves the SPA entirely through the LFS OAuth redirect.
 */
export class Session {
  /** Reads through to the root layout's data, so a reload is picked up here. */
  readonly #me: () => MeResponse;
  #refreshed = $state.raw<{ source: MeResponse; value: MeResponse } | null>(
    null,
  );
  #refreshing = false;

  /** Set while a sign-out request is in flight. */
  pending = $state(false);
  error = $state("");

  constructor(
    me: () => MeResponse,
    private readonly api: ApiClient,
  ) {
    this.#me = me;
  }

  get me(): MeResponse {
    const source = this.#me();
    return this.#refreshed?.source === source ? this.#refreshed.value : source;
  }

  /** Refresh frontend policy without reloading page data; retain state on failure. */
  refresh = async () => {
    if (this.#refreshing || this.pending) return;
    this.#refreshing = true;
    const source = this.#me();
    try {
      const value = await this.api.authentication.get();
      if (!this.pending && this.#me() === source) {
        this.#refreshed = { source, value };
      }
    } catch {
      // A transient failure must not clear the notice or change permissions.
    } finally {
      this.#refreshing = false;
    }
  };

  get signedIn(): boolean {
    return this.me.authenticated;
  }

  get player(): PlayerSummary | null {
    return this.me.player;
  }

  /** The name to show in the header for the current player. */
  get displayName(): string {
    return this.me.player?.display_name ?? this.me.player?.lfs_username ?? "";
  }

  /** Hands the browser to LFS OAuth, returning here once it completes. */
  signIn = () => {
    window.location.href = "/auth/lfs";
  };

  /**
   * Clears the server session, then reloads every load function so the whole
   * page drops back to its signed-out shape in one step.
   */
  signOut = async () => {
    if (this.pending) return;
    this.pending = true;
    this.error = "";
    try {
      await this.api.authentication.logout({
        "X-CSRF-Token": this.me.csrf_token,
      });
      try {
        await refreshAll();
      } catch {
        this.error =
          "You signed out, but the page could not refresh. Please reload.";
      }
    } catch (cause) {
      this.error = apiErrorMessage(cause);
    } finally {
      this.pending = false;
    }
  };
}

/** Publishes the session loaded by the root layout to the rest of the tree. */
export function setSession(me: () => MeResponse, api: ApiClient) {
  return setContext(KEY, new Session(me, api));
}

export function useSession() {
  return getContext<Session>(KEY);
}
