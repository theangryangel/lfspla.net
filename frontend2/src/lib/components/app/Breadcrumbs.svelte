<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as Breadcrumb from '$lib/components/ui/breadcrumb/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import Flag from '$lib/components/app/Flag.svelte';
	import type { Crumb } from '$lib/breadcrumbs.js';

	/**
	 * The trail the matched routes contributed through their loads.
	 *
	 * `trailing` renders inside the last crumb, for the controls that belong to
	 * the page you are on rather than to the trail.
	 */
	let {
		items = [],
		class: className,
		actions = {},
		trailing,
	}: {
		items?: Crumb[];
		class?: string;
		actions?: Record<string, (from: HTMLButtonElement) => void>;
		trailing?: Snippet;
	} = $props();
</script>

{#if items.length}
	<Breadcrumb.Root class="min-w-0 {className ?? ''}">
		<Breadcrumb.List class="flex-nowrap">
			{#each items as crumb, index (index)}
				{@const current = index === items.length - 1}
				{#if index > 0}
					<Breadcrumb.Separator />
				{/if}
				<Breadcrumb.Item>
					{#if current}
						<Breadcrumb.Page
							class="flex min-w-0 items-center gap-2 font-semibold"
						>
							{#if crumb.flag}<Flag code={crumb.flag} />{/if}
							<span class="truncate">{crumb.label}</span>
						</Breadcrumb.Page>
					{:else if crumb.href}
						<Breadcrumb.Link
							class="flex min-w-0 items-center gap-2 font-medium"
							href={crumb.href}
						>
							{#if crumb.flag}<Flag code={crumb.flag} />{/if}
							<span class="truncate">{crumb.label}</span>
						</Breadcrumb.Link>
					{:else if crumb.action && actions[crumb.action]}
						{@const run = actions[crumb.action]}
						<button
							type="button"
							aria-haspopup="dialog"
							onclick={(event) => run(event.currentTarget)}
							class="flex min-w-0 cursor-pointer items-center gap-2 font-medium transition-colors outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
						>
							<span class="truncate">{crumb.label}</span>
						</button>
					{:else}
						<span class="truncate font-medium">{crumb.label}</span>
					{/if}
					{#if crumb.tag}
						<Badge
							variant="outline"
							class={crumb.tag.accent ? 'border-time-pb/40 text-time-pb' : ''}
						>
							{crumb.tag.label}
						</Badge>
					{/if}
					{#if current && trailing}{@render trailing()}{/if}
				</Breadcrumb.Item>
			{/each}
		</Breadcrumb.List>
	</Breadcrumb.Root>
{/if}
