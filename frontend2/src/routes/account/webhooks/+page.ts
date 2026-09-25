import {
  get,
  getList,
  query,
  type PaginatedResponse,
  type WebhookNotificationResponse,
  type WebhookResponse,
  type WebhookOptionsResponse,
} from "$lib/api.js";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, parent, url }) => {
  const { me, breadcrumbs } = await parent();
  const [webhooks, options, notifications] = me.authenticated
    ? await Promise.all([
        getList<WebhookResponse>(fetch, "/api/v1/me/webhooks"),
        get<WebhookOptionsResponse>(fetch, "/api/v1/me/webhooks/options"),
        get<PaginatedResponse<WebhookNotificationResponse>>(
          fetch,
          "/api/v1/me/webhooks/notifications" +
            query({ page: url.searchParams.get("page") ?? 1, per_page: 20 }),
        ),
      ])
    : [[], null, null];
  return {
    breadcrumbs: [
      ...breadcrumbs,
      { label: "Webhooks", href: "/account/webhooks" },
    ],
    webhooks,
    options,
    notifications,
  };
};
