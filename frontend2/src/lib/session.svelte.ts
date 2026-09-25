import { getContext, setContext } from "svelte";
import { invalidateAll } from "$app/navigation";
import { send, type MeResponse, type PlayerSummary } from "$lib/api.js";

const KEY = Symbol("lfsplanet.session");

/**
 * The signed-in player, as `GET /api/v1/me` reported them for this navigation.
 *
 * The root layout load owns the fetch; this only holds the answer and performs
 * the two transitions out of it. Signing in leaves the SPA entirely (the LFS
 * OAuth redirect), so there is no client-side login state to keep in step.
 */
export class Session {
  /** Reads through to the root layout's data, so a reload is picked up here. */
  readonly #me: () => MeResponse;
  /** Set while a sign-out request is in flight. */
  pending = $state(false);
  error = $state("");

  constructor(me: () => MeResponse) {
    this.#me = me;
  }

  get me(): MeResponse {
    return this.#me();
  }

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
      await send("/auth/logout", { method: "POST", csrf: this.me.csrf_token });
      try {
        await invalidateAll();
      } catch {
        this.error =
          "You signed out, but the page could not refresh. Please reload.";
      }
    } catch (cause) {
      this.error =
        cause instanceof Error ? cause.message : "You could not be signed out.";
    } finally {
      this.pending = false;
    }
  };
}

/** Publishes the session loaded by the root layout to the rest of the tree. */
export function setSession(me: () => MeResponse) {
  return setContext(KEY, new Session(me));
}

export function useSession() {
  return getContext<Session>(KEY);
}
