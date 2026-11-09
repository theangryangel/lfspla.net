import { LfsplanetApiError } from "@lfsplanet/sdk";
import { createApi } from "#lib/api.js";
import { error, redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({
  params,
  fetch,
  url,
  depends,
  parent,
}) => {
  if (
    !/^[1-9][0-9]*$/.test(params.id) ||
    !Number.isSafeInteger(Number(params.id))
  )
    error(404, "Event not found");
  const api = createApi(fetch);
  depends("app:event");
  const event = await api.events
    .getEvent({ event_id: Number(params.id) })
    .catch((cause: unknown) => {
      if (cause instanceof LfsplanetApiError && cause.statusCode === 404)
        error(404, "Event not found");
      throw cause;
    });
  if (event.source === "lfs_net" && event.source_url) {
    const target = new URL(event.source_url);
    if (
      target.protocol === "https:" &&
      (target.hostname === "www.lfs.net" || target.hostname === "lfs.net")
    )
      redirect(307, target.href);
    error(502, "The upstream event link is unavailable");
  }
  const page = Number(url.searchParams.get("page") ?? 1);
  if (!Number.isSafeInteger(page) || page < 1)
    error(400, "Invalid results page");
  const results = await api.events.eventResults({
    event_id: event.id,
    page,
    per_page: 50,
  });
  const { breadcrumbs } = await parent();
  return {
    event,
    results,
    breadcrumbs: [
      ...breadcrumbs,
      { label: event.title, href: `/events/${event.id}` },
    ],
  };
};
