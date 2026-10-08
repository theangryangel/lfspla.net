# SDKs

Pre-created clients to access lfspla.net.

## Regenerate

After changing the Rust API, run:

```sh
just generate-sdks
```

Requires Rust, Node/npm, Docker, and `npm --prefix frontend2 ci`. Commit the
generated source with the API change. CI checks for stale output.

The TypeScript generator is pinned to **3.75.0** because 3.75.1 introduced a
TypeScript compatibility regression. Check
[Fern PR #17817](https://github.com/fern-api/fern/pull/17817) before upgrading.

## Use

In a route load, pass SvelteKit's `fetch` so its request invalidation works:

```ts
import { createApi } from "$lib/api.js";

const api = createApi(fetch);
const eras = await api.eras.listEras();
```

In components, use the root-layout client:

```ts
import { useApi } from "$lib/api.js";

const api = useApi();
await api.hotlaps.listHotlaps({ page: 1 });
```

Both helpers are exported from `$lib/api.js`. Responses are typed; the client
does not validate response JSON at runtime.
