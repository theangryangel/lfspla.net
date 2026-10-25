import { derived, writable } from "svelte/store";

const pending = writable(0);
export const loading = derived(pending, (count) => count > 0);

/** Each operation owns its completion, including overlapping and aborted requests. */
export function beginLoading(): () => void {
  pending.update((count) => count + 1);
  let finished = false;
  return () => {
    if (finished) return;
    finished = true;
    pending.update((count) => count - 1);
  };
}

export function progressFetch(fetch: typeof globalThis.fetch): typeof fetch {
  return async (input, init) => {
    const finish = beginLoading();
    try {
      const response = await fetch(input, init);
      // Fetch resolves at headers. Drain a clone to track the full download,
      // leaving the original response and its metadata intact for the SDK.
      const reader = response.clone().body?.getReader();
      if (!reader) {
        finish();
      } else {
        void (async () => {
          try {
            while (!(await reader.read()).done) {
              // Discard each chunk immediately rather than buffering a second body.
            }
          } catch {
            // The SDK consumes the original body and reports download errors.
          } finally {
            reader.releaseLock();
            finish();
          }
        })();
      }
      return response;
    } catch (cause) {
      finish();
      throw cause;
    }
  };
}
