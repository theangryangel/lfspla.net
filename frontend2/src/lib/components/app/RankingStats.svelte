<script lang="ts">
	import CircleHelpIcon from '@lucide/svelte/icons/circle-help';
	import { Button } from '$lib/components/ui/button/index.js';
	import type { RankingDetailResponse } from '$lib/api.js';

	let {
		ranking,
		progressHref,
	}: { ranking: RankingDetailResponse; progressHref?: string } = $props();
</script>

<div class="flex flex-wrap items-center gap-6">
	<dl class="flex flex-wrap gap-6 text-sm">
		<div class="flex items-baseline gap-2">
			<dt class="text-muted-foreground">Charts</dt>
			<dd class="font-semibold tabular-nums">
				{ranking.charts.length.toLocaleString()}
			</dd>
		</div>
		<div class="flex items-baseline gap-2">
			<dt class="text-muted-foreground">Benchmark</dt>
			<dd class="font-semibold tabular-nums">
				{ranking.rules.benchmark_percent}%
			</dd>
		</div>
	</dl>

	{#if progressHref}
		<Button variant="outline" href={progressHref}>
			<CircleHelpIcon />
			Rank Info
		</Button>
	{/if}
</div>
