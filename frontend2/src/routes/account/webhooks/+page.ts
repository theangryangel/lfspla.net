import { createApi } from "$lib/api.js";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, parent, url }) => {
  const api = createApi(fetch);
  const { me, breadcrumbs } = await parent();
  const [webhooks, options, notifications] = me.authenticated
    ? await Promise.all([
        api.webhooks.listWebhooks().then((response) => response.items),
        api.webhooks.webhookOptions(),
        api.webhooks.listWebhookNotifications({
          page:
            (url.searchParams.get("page")
              ? Number(url.searchParams.get("page"))
              : undefined) ?? 1,
          per_page: 20,
        }),
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
