import { goto } from "$app/navigation";
import { page } from "$app/state";

export function setQuery(values: Record<string, string>, replaceState = false) {
  const url = new URL(page.url.href);
  for (const [key, value] of Object.entries(values)) {
    if (value) url.searchParams.set(key, value);
    else url.searchParams.delete(key);
  }
  return goto(url, {
    replace: replaceState,
    reset: false,
  });
}
