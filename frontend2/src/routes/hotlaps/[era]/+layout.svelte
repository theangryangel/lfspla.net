<script lang="ts">
	import type { Snippet } from 'svelte';
	import { goto, afterNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import CombinationProvider from '$lib/components/app/CombinationProvider.svelte';
	import PageNavigation from '$lib/components/app/PageNavigation.svelte';
	import Breadcrumbs from '$lib/components/app/Breadcrumbs.svelte';
	import { navigationTextClass } from '$lib/components/app/navigation.js';
	import HotlapUpload from '$lib/components/app/HotlapUpload.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { hotlapPath } from '$lib/era.js';
	import type { LayoutProps } from './$types';

	let { data, children }: LayoutProps & { children: Snippet } = $props();
	const base = $derived(hotlapPath(data.era.id));
	const navigation = $derived([
		{ label: 'Overview', to: base, exact: true },
		{ key: 'charts', label: 'Charts', content: chartsButton },
		{ label: 'Ranks', to: `${base}/rankings` },
		{ label: 'My Submissions', to: `${base}/submissions` },
	]);
	let picker = $state<CombinationProvider | null>(null);
	const onChart = $derived(
		page.route.id === '/hotlaps/[era]/charts/[track]/[vehicle]',
	);
	// Support bookmarks to the former chart picker entry point.
	afterNavigate(() => {
		if (page.url.searchParams.get('picker') === 'charts') openPicker();
	});

	/** Opens on the combination in view, so a chart starts where it already is. */
	function openPicker(from?: HTMLElement | null) {
		picker?.open({
			track: onChart ? page.data.track : null,
			vehicle: onChart ? page.data.vehicle : null,
			from,
		});
	}

	function showChart(track: string, vehicle: string) {
		const filters = new URLSearchParams();
		if (onChart) {
			for (const key of ['country', 'controller', 'per_page']) {
				const value = page.url.searchParams.get(key);
				if (value !== null) filters.set(key, value);
			}
		}
		const search = filters.toString();
		return goto(
			`${base}/charts/${encodeURIComponent(track)}/${encodeURIComponent(vehicle)}${search ? `?${search}` : ''}`,
		);
	}
</script>

{#snippet chartsButton()}
	<button
		type="button"
		data-active={onChart ? '' : undefined}
		onclick={(event) => openPicker(event.currentTarget)}
		class="flex h-8 cursor-pointer items-center gap-2 rounded-lg px-2.5 py-0 text-sm transition-all outline-none focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-1 {navigationTextClass}"
	>
		Charts
	</button>
{/snippet}

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<div class="flex flex-wrap items-center justify-between gap-4">
		<div class="flex w-full min-w-0 flex-wrap items-center gap-3 md:w-auto">
			<Breadcrumbs
				items={page.data.breadcrumbs}
				actions={{ charts: openPicker }}
			/>
		</div>
		<PageNavigation label="Era" items={navigation}>
			{#snippet action()}
				<HotlapUpload era={data.era} />
			{/snippet}
		</PageNavigation>
	</div>
	<CombinationProvider
		bind:this={picker}
		era={data.era.id}
		onSelect={showChart}
	>
		{@render children()}
	</CombinationProvider>
</main>
