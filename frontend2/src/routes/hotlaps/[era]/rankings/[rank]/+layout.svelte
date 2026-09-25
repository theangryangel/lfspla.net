<script lang="ts">
	import { page } from '$app/state';
	import RankingStats from '$lib/components/app/RankingStats.svelte';
	import type { Snippet } from 'svelte';
	import type { LayoutProps } from './$types';

	let { data, children }: LayoutProps & { children: Snippet } = $props();
</script>

{#if page.route.id !== '/hotlaps/[era]/rankings/[rank]' && page.route.id !== '/hotlaps/[era]/rankings/[rank]/combinations'}
	<div class="flex justify-end">
		<RankingStats
			ranking={data.ranking}
			progressHref={`/hotlaps/${data.era.id}/rankings/${data.ranking.id}/combinations`}
		/>
	</div>
{/if}

{@render children()}
