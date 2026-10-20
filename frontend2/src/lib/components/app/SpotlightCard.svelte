<script lang="ts">
	import type { SpotlightResponse } from '$lib/api.js';
	import Flag from './Flag.svelte';
	import Thumbnail from './Thumbnail.svelte';
	import RecordHighlight from './RecordHighlight.svelte';
	import { lapTime } from '$lib/format.js';

	let { card }: { card: SpotlightResponse } = $props();
	const content = $derived(card.content);
	const href = $derived(
		content.type === 'driver'
			? `/drivers/${encodeURIComponent(content.player.lfs_username)}`
			: `/hotlaps/${encodeURIComponent(content.chart.era_id)}/charts/${encodeURIComponent(content.chart.track.code)}/${encodeURIComponent(content.chart.vehicle.code)}`,
	);
</script>

<div
	class="grid min-h-30 items-stretch bg-card sm:grid-cols-[minmax(0,1fr)_auto]"
>
	<a
		{href}
		class="flex min-w-0 items-center gap-3 px-5 py-5 hover:text-primary focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-[-2px]"
	>
		{#if content.type === 'driver'}
			<Flag
				code={content.player.flag_code}
				fallback={content.player.country_code}
			/>
			<div class="min-w-0">
				<p class="break-words text-xl font-semibold">
					{content.player.display_name}
				</p>
			</div>
		{:else}
			<div class="grid min-w-0 flex-1 items-center gap-4 sm:grid-cols-3">
				<p class="racing-section-title break-words">
					{content.chart.era_title}
				</p>
				<div class="flex min-w-0 items-center gap-2">
					<Thumbnail
						src={content.chart.track.image?.url ??
							content.chart.track.outline?.url}
						class="w-14 shrink-0 bg-transparent"
					/>
					<div class="min-w-0">
						<h3 class="racing-section-title break-words">
							{content.chart.track.name}
						</h3>
						<p class="mt-1 text-sm text-muted-foreground">
							{content.chart.track.code}
						</p>
					</div>
				</div>
				<div class="flex min-w-0 items-center gap-2">
					<Thumbnail
						src={content.chart.vehicle.image?.url}
						class="w-14 shrink-0 bg-transparent"
					/>
					<div class="min-w-0">
						<h3 class="racing-section-title break-words">
							{content.chart.vehicle.name}
						</h3>
						<p class="mt-1 text-sm text-muted-foreground">
							{content.chart.vehicle.code}
						</p>
					</div>
				</div>
			</div>
		{/if}
	</a>
	<RecordHighlight
		title={card.highlight_label}
		variant={content.type === 'driver' ? 'cyan' : 'purple'}
		compact
		flushEnd
		class="content-center py-4 text-right lg:w-auto lg:py-4"
	>
		<p class="text-right font-mono text-2xl font-bold tabular-nums">
			{card.milliseconds
				? `+${lapTime(card.highlight_value)}`
				: card.percentage
					? `${(card.highlight_value / 100).toLocaleString(undefined, { maximumFractionDigits: 2 })}%`
					: card.highlight_value.toLocaleString()}
		</p>
	</RecordHighlight>
</div>
