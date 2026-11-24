<script lang="ts">
	import type { EraSummary, RankingContribution } from '#lib/api.js';
	let {
		contributions,
		era,
	}: { contributions: RankingContribution[]; era: EraSummary } = $props();
</script>

<section class="min-w-0" aria-label="Contributing rankings">
	<h2 class="mb-2 text-sm font-semibold text-muted-foreground">
		Contributes to
	</h2>
	{#if contributions.length}
		<ul class="flex flex-wrap gap-x-4 gap-y-2">
			{#each contributions as ranking (ranking.id)}
				<li>
					<a
						href={`/hotlaps/${encodeURIComponent(era.id)}/rankings/${encodeURIComponent(ranking.id)}`}
						title={era.rankings.find((candidate) => candidate.id === ranking.id)
							?.description}
						class="text-sm font-medium underline decoration-border underline-offset-4 hover:decoration-current focus-visible:rounded-sm focus-visible:outline-2 focus-visible:outline-ring"
						>{ranking.title}</a
					>
				</li>
			{/each}
		</ul>
	{:else}<p class="text-sm text-muted-foreground">
			No rankings include this chart.
		</p>{/if}
</section>
