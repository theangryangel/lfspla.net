<script lang="ts">
	import type { EraSummary, EraStatsResponse } from '#lib/api.js';
	import PageIdentity from './PageIdentity.svelte';
	import RecordHighlight from './RecordHighlight.svelte';
	let {
		era,
		stats = null,
	}: { era: EraSummary; stats?: EraStatsResponse | null } = $props();
	const highlights = $derived([
		{
			label: 'Hotlaps',
			value: stats?.validated_hotlaps,
			variant: 'cyan' as const,
		},
		{ label: 'Rankings', value: stats?.rankings, variant: 'purple' as const },
		{ label: 'Charts', value: stats?.charts, variant: 'yellow' as const },
	]);
</script>

<PageIdentity label="Era identity">
	<div class="relative space-y-2 py-1">
		<div class="flex flex-wrap items-center gap-3">
			<h1 class="racing-page-title">{era.title}</h1>
		</div>
		<p class="max-w-prose text-sm text-muted-foreground">
			{#if era.open}Replays recorded with LFS {era.version_requirement}.
			{:else}Records from LFS {era.version_requirement}. Closed for uploads.{/if}
		</p>
	</div>
	{#snippet highlight()}
		<div class="era-highlights grid min-w-0 grid-cols-3 lg:w-[36rem]">
			{#each highlights as stat (stat.label)}
				<RecordHighlight
					title={stat.label}
					variant={stat.variant}
					compact
					class="w-auto px-6 lg:w-auto lg:px-8"
				>
					<p class="text-right text-2xl font-bold tabular-nums">
						{stat.value == null ? '—' : stat.value.toLocaleString()}
					</p>
				</RecordHighlight>
			{/each}
		</div>
	{/snippet}
</PageIdentity>

<style>
	@media (min-width: 64rem) {
		.era-highlights {
			padding-left: var(--highlight-slant);
		}
		.era-highlights :global(.record-highlight) {
			margin-left: calc(-1 * var(--highlight-slant));
		}
	}
</style>
