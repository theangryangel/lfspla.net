<script lang="ts">
	import FloatingPanel from './FloatingPanel.svelte';
	import GitCompareArrows from '@lucide/svelte/icons/git-compare-arrows';
	import X from '@lucide/svelte/icons/x';
	import { page } from '$app/state';
	import { Button } from '$lib/components/ui/button/index.js';
	import Flag from './Flag.svelte';
	import { useComparison } from '$lib/compare.svelte.js';
	import { query } from '$lib/api.js';
	const comparison = useComparison();
	const visible = $derived(
		comparison.drivers.length > 0 && page.url.pathname !== '/compare',
	);
	const href = $derived(
		'/compare' +
			query({
				left: comparison.drivers[0]?.lfs_username,
				right: comparison.drivers[1]?.lfs_username,
				era: page.params.era ?? page.data.currentEraId,
			}),
	);
</script>

{#if visible}
	<FloatingPanel order={10}>
		<aside
			aria-label="Driver comparison"
			class="rounded-lg border border-border bg-popover/95 p-3 text-popover-foreground shadow-lg backdrop-blur-xl sm:px-4"
		>
			<div class="flex flex-col gap-3 sm:flex-row sm:items-center">
				<div class="flex shrink-0 items-center gap-2 text-sm font-semibold">
					<GitCompareArrows class="size-4 text-primary" aria-hidden="true" />
					Compare drivers
				</div>
				<div
					class="flex min-w-0 flex-1 gap-2 overflow-x-auto"
					aria-live="polite"
				>
					{#each comparison.drivers as driver (driver.lfs_username)}
						<div
							class="flex shrink-0 items-center gap-2 rounded-md border border-border bg-background/60 py-1 pr-1 pl-2 text-sm"
						>
							<Flag code={driver.country_code} />
							<span class="max-w-40 truncate" title={driver.display_name}
								>{driver.display_name}</span
							>
							<Button
								variant="ghost"
								size="icon-sm"
								class="size-6"
								aria-label={`Remove ${driver.display_name} from comparison`}
								onclick={() => comparison.remove(driver.lfs_username)}
								><X class="size-3.5" aria-hidden="true" /></Button
							>
						</div>
					{/each}
				</div>
				<div class="flex shrink-0 items-center justify-end gap-2">
					<Button variant="ghost" onclick={() => comparison.seed([])}
						>Clear</Button
					>
					{#if comparison.drivers.length === 2}<Button {href}>Compare</Button
						>{:else}<Button disabled>Compare</Button>{/if}
				</div>
			</div>
		</aside>
	</FloatingPanel>
{/if}
