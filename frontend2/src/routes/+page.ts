import { createApi } from "#lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ depends, fetch }) => {
  const api = createApi(fetch);
  depends("app:hotlaps");
  const now = Date.now();
  const eventWindow = {
    starts_from: new Date(now).toISOString(),
    starts_before: new Date(now + 7 * 24 * 60 * 60 * 1000).toISOString(),
    per_page: 100,
  };
  async function upcomingEvents() {
    const first = await api.events.listEvents({ ...eventWindow, page: 1 });
    const items = [...first.items];
    for (let page = 2; page <= first.pagination.total_pages; page += 1) {
      const response = await api.events.listEvents({ ...eventWindow, page });
      items.push(...response.items);
    }
    return items;
  }
  const [stats, authoredCards, events] = await Promise.all([
    api.stats.getStats().catch(() => null),
    api.communityCards
      .listCommunityCards()
      .then((response) => response.items)
      .catch(() => null),
    upcomingEvents().catch(() => null),
  ]);
  // Fisher–Yates gives every ordering equal probability without retaining state.
  function shuffled<T>(items: readonly T[]): T[] {
    const result = [...items];
    for (let index = result.length - 1; index > 0; index -= 1) {
      const other = Math.floor(Math.random() * (index + 1));
      [result[index], result[other]] = [result[other], result[index]];
    }
    return result;
  }
  const communityCards = shuffled(authoredCards ?? []);
  return {
    stats: stats ? { ...stats, spotlights: shuffled(stats.spotlights) } : null,
    communityCards,
    upcomingEvents: events,
  };
};
