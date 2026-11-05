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
	class={`min-w-0 scroll-mt-6 rounded-lg border bg-card text-card-foreground target:ring-3 ${report ? (resolved ? 'border-destructive/25 target:ring-destructive/10' : 'border-destructive/60 target:ring-destructive/20') : 'target:ring-primary/30'}`}
>
	<header class="flex items-start justify-between gap-3 px-4 pt-4">
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
	<div class="min-w-0 space-y-2 px-4 pt-2 pb-4">{@render children()}</div>
</article>
