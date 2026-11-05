<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { PlayerSummary } from '#lib/api.js';
	import DateTime from './DateTime.svelte';
	import Flag from './Flag.svelte';
	let {
		author,
		createdAt,
		id,
		children,
		actions,
		report = false,
		resolved = false,
	}: {
		author: PlayerSummary;
		createdAt: string;
		id: string;
		children: Snippet;
		actions?: Snippet;
		report?: boolean;
		resolved?: boolean;
	} = $props();
</script>

<article
	{id}
	class={`min-w-0 scroll-mt-6 border-y text-card-foreground target:bg-primary/5 ${report ? (resolved ? 'border-l-4 border-l-brand-yellow/50 bg-brand-yellow/5' : 'border-l-4 border-l-brand-yellow bg-brand-yellow/10') : 'bg-card/30'}`}
>
	<header
		class={`flex items-start justify-between gap-3 px-4 py-2 ${report ? 'bg-brand-yellow/10' : 'bg-muted/20'}`}
	>
		<div class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs">
			<a
				href={`/drivers/${encodeURIComponent(author.lfs_username)}`}
				class="inline-flex min-w-0 items-center gap-2 text-sm font-semibold hover:underline"
			>
				<Flag code={author.flag_code} fallback={author.country_code} />
				<span class="break-all"
					>{author.display_name || author.lfs_username}</span
				>
			</a>
			<span class="text-muted-foreground"><DateTime value={createdAt} /></span>
		</div>
		{#if actions}<div class="-my-1 -mr-2 shrink-0">
				{@render actions()}
			</div>{/if}
	</header>
	<div class="min-w-0 space-y-2 px-4 pt-2 pb-3">{@render children()}</div>
</article>
