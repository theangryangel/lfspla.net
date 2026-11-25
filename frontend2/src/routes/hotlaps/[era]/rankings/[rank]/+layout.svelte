<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import type { LayoutProps } from './$types';
	let { data, children }: LayoutProps = $props();
	const base = $derived(
		`/hotlaps/${encodeURIComponent(data.era.id)}/rankings/${encodeURIComponent(data.ranking.id)}`,
	);
	const view = $derived(
		page.route.id?.endsWith('/combinations') ? 'combinations' : 'standings',
	);
	function changeView(value: string) {
		if (value === view || (value !== 'standings' && value !== 'combinations'))
			return;
		void goto(value === 'combinations' ? `${base}/combinations` : base, {
			reset: false,
		});
	}
</script>

<Tabs.Root value={view} onValueChange={changeView} class="min-w-0 gap-4">
	<Tabs.List aria-label="Ranking views">
		<Tabs.Trigger value="standings">Standings</Tabs.Trigger>
		<Tabs.Trigger value="combinations">Rank info &amp; progress</Tabs.Trigger>
	</Tabs.List>
	<Tabs.Content value={view}>
		{@render children()}
	</Tabs.Content>
</Tabs.Root>
