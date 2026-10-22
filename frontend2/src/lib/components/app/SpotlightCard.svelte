<script lang="ts">
	import type { SpotlightResponse } from '#lib/api.js';
	import Flag from './Flag.svelte';
	import Thumbnail from './Thumbnail.svelte';
	import RecordHighlight from './RecordHighlight.svelte';
	import { lapTime } from '#lib/format.js';

	let { card }: { card: SpotlightResponse } = $props();
	const content = $derived(card.content);
	const unclaimed = $derived(card.id === 'be-the-first');
	const href = $derived(
		content.type === 'driver'
			? `/drivers/${encodeURIComponent(content.player.lfs_username)}`
			: `/hotlaps/${encodeURIComponent(content.chart.era_id)}/charts/${encodeURIComponent(content.chart.track.code)}/${encodeURIComponent(content.chart.vehicle.code)}`,
	);
</script>

<div
	class="grid min-h-28 items-stretch bg-card sm:grid-cols-[minmax(0,1fr)_auto]"
>
	<a
		{href}
		class="group flex min-w-0 items-center gap-3 px-4 py-3 hover:bg-muted/50 focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-[-2px]"
	>
		{#if content.type === 'driver'}
			<div class="flex min-w-0 flex-1 items-center justify-center gap-3">
				<Flag
					code={content.player.flag_code}
					fallback={content.player.country_code}
				/>
				<p class="text-xl font-semibold wrap-anywhere group-hover:text-primary">
					{content.player.display_name}
				</p>
			</div>
		{:else}
			<div class="grid min-w-0 flex-1 self-stretch grid-rows-[1fr_auto_1fr]">
				<div
					class="row-start-2 flex min-w-0 flex-wrap items-center justify-center gap-x-4 gap-y-2"
				>
					<div class="flex min-w-0 max-w-full items-center gap-3">
						<Thumbnail
							src={content.chart.track.image?.url ??
								content.chart.track.outline?.url}
							class="w-12 shrink-0 bg-transparent"
						/>
						<p
							class="min-w-0 text-lg leading-snug font-semibold wrap-anywhere group-hover:text-primary"
						>
							{content.chart.track.name}
							<span
								class="ml-1 font-mono text-sm font-normal text-muted-foreground"
								>{content.chart.track.code}</span
							>
						</p>
					</div>
					<div class="flex min-w-0 max-w-full items-center gap-3">
						<Thumbnail
							src={content.chart.vehicle.image?.url}
							class="w-12 shrink-0 bg-transparent"
						/>
						<p
							class="min-w-0 text-lg leading-snug font-semibold wrap-anywhere group-hover:text-primary"
						>
							{content.chart.vehicle.name}
							<span
								class="ml-1 font-mono text-sm font-normal text-muted-foreground"
								>{content.chart.vehicle.code}</span
							>
						</p>
					</div>
				</div>
				<p
					class="row-start-3 self-end text-right text-sm text-muted-foreground wrap-anywhere"
				>
					{content.chart.era_title}
				</p>
			</div>
		{/if}
	</a>

	<RecordHighlight
		title={unclaimed ? 'Record' : card.highlight_label}
		variant={content.type === 'driver' ? 'cyan' : 'purple'}
		compact
		flushEnd
		class="content-center py-4 text-right lg:w-auto lg:py-4"
	>
		<p
			class={[
				'text-right font-bold',
				unclaimed ? 'text-xl' : 'font-mono text-2xl tabular-nums',
			]}
		>
			{#if unclaimed}
				Unclaimed
			{:else}
				{card.milliseconds
					? `+${lapTime(card.highlight_value)}`
					: card.percentage
						? `${(card.highlight_value / 100).toLocaleString(undefined, { maximumFractionDigits: 2 })}%`
						: card.highlight_value.toLocaleString()}
			{/if}
		</p>
	</RecordHighlight>
</div>
