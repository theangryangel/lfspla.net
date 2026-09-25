import { goto } from "$app/navigation";
import { page } from "$app/state";

/**
 * Filters, sort, pagination and the selected view all live in the URL, so links
 * and reloads restore exactly what the reader was looking at.
 */

/** Keys that move within a result set rather than changing what it contains. */
const POSITION = new Set([
  "page",
  "wr_page",
  "nation_page",
  "selected",
  "view",
]);

/** Applies one query-string change and navigates to the result. */
export function updateQuery(key: string, value: string, replaceState = false) {
  return setQuery({ [key]: value }, replaceState);
}

/**
 * Applies several query-string changes in one navigation.
 *
 * Changing a filter or page size returns to the first page.
 */
export function setQuery(values: Record<string, string>, replaceState = false) {
  const next = new URLSearchParams(page.url.searchParams);
  for (const [key, value] of Object.entries(values)) {
    if (value) next.set(key, value);
    else next.delete(key);
  }
  if (Object.keys(values).some((key) => !POSITION.has(key))) {
    next.delete("page");
  }

  const search = next.toString();
  return goto(`${page.url.pathname}${search ? `?${search}` : ""}`, {
    replaceState,
    keepFocus: true,
    noScroll: true,
  });
}

export function queryValue(key: string) {
  return page.url.searchParams.get(key) ?? "";
}
