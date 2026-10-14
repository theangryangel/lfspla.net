<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { PersonalRankingEntry, NationRankingEntry } from '$lib/api.js';
	import { goto, afterNavigate, replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import CombinationProvider from '$lib/components/app/CombinationProvider.svelte';
	import PageNavigation from '$lib/components/app/PageNavigation.svelte';
	import RankingPicker from '$lib/components/app/RankingPicker.svelte';
	import RecordHighlight from '$lib/components/app/RecordHighlight.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import PageIdentity from '$lib/components/app/PageIdentity.svelte';
	import CircleHelpIcon from '@lucide/svelte/icons/circle-help';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import EraIdentity from '$lib/components/app/EraIdentity.svelte';
	import CombinationCard from '$lib/components/app/CombinationCard.svelte';
	import { lapTime, delta } from '$lib/format.js';
	import Breadcrumbs from '$lib/components/app/Breadcrumbs.svelte';
	import { navigationTextClass } from '$lib/components/app/navigation.js';
	import HotlapUpload from '$lib/components/app/HotlapUpload.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import type { LayoutProps } from './$types';

	let { data, children }: LayoutProps & { children: Snippet } = $props();
	const base = $derived(`/hotlaps/${data.era.id}`);
	const navigation = $derived([
		{ label: 'Overview', to: base, exact: true },
		{ key: 'charts', label: 'Charts', content: chartsButton },
		{ key: 'rankings', label: 'Ranks', content: ranksButton },
		{ label: 'My Submissions', to: `${base}/submissions` },
	]);
	let rankingPicker = $state<RankingPicker | null>(null);
	const onRanking = $derived(
		page.route.id?.startsWith('/hotlaps/[era]/rankings/[rank]') ?? false,
	);
	const driverLeaders = $derived(
		onRanking
			? ((page.data.players?.entries ?? []) as PersonalRankingEntry[]).filter(
					(row) => row.position === 1,
				)
			: [],
	);
	const nationLeaders = $derived(
		onRanking
			? ((page.data.nations?.entries ?? []) as NationRankingEntry[]).filter(
					(row) => row.position === 1,
				)
			: [],
	);
	let picker = $state<CombinationProvider | null>(null);
	const onChart = $derived(
		page.route.id === '/hotlaps/[era]/charts/[track]/[vehicle]',
	);
	const onOverview = $derived(page.route.id === '/hotlaps/[era]');
	const chartRecord = $derived.by(() => {
		if (!onChart) return null;
		const entry = page.data.chart?.items[0];
		return entry && entry.distance_to_world_record_ms !== null
			? lapTime(entry.lap_time_ms - entry.distance_to_world_record_ms)
			: null;
	});
	// Support bookmarks to the former chart picker entry point.
	afterNavigate(() => {
		if (page.url.searchParams.get('picker') === 'charts') openPicker();
		if (page.url.searchParams.get('picker') === 'rankings') {
			rankingPicker?.open(null, page.url.searchParams.get('q') ?? '');
			const url = new URL(page.url);
			url.searchParams.delete('picker');
			url.searchParams.delete('q');
			replaceState(url, page.state);
		}
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

{#snippet ranksButton()}
	<button
		type="button"
		aria-haspopup="dialog"
		data-active={onRanking ? '' : undefined}
		onclick={(event) => rankingPicker?.open(event.currentTarget)}
		class="flex h-8 cursor-pointer items-center gap-2 rounded-lg px-2.5 text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50 {navigationTextClass}"
		>Ranks</button
	>
{/snippet}

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

{#snippet eraContext()}
	<div class="flex flex-wrap items-center justify-between gap-4">
		<div class="flex w-full min-w-0 flex-wrap items-center gap-3 md:w-auto">
			<Breadcrumbs
				class="[&_ol]:flex-wrap sm:[&_ol]:flex-nowrap"
				items={page.data.breadcrumbs}
				actions={{
					charts: openPicker,
					rankings: (from) => rankingPicker?.open(from),
				}}
			/>
		</div>
		<PageNavigation label="Era" items={navigation}>
			{#snippet action()}<HotlapUpload era={data.era} />{/snippet}
		</PageNavigation>
	</div>
{/snippet}

<CombinationProvider bind:this={picker} era={data.era.id} onSelect={showChart}>
	<main id="main" class="w-full min-w-0 flex-1">
		{#if onChart || onOverview || onRanking}
			<div class="mx-auto w-full max-w-screen-2xl p-4 md:p-6">
				{@render eraContext()}
			</div>
			{#if onOverview}
				<EraIdentity era={data.era} leaders={page.data.leaders} />
			{:else if onRanking}
				<PageIdentity label="Ranking identity">
					<div class="flex min-h-19 flex-col justify-center py-2">
						<h1 class="min-w-0">
							<button
								type="button"
								aria-haspopup="dialog"
								aria-label={page.data.ranking.title + ': choose a ranking'}
								onclick={(event) => rankingPicker?.open(event.currentTarget)}
								class="flex max-w-full cursor-pointer items-center gap-3 rounded-sm py-1 text-left transition-colors hover:bg-muted/50 focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-4"
							>
								<span class="racing-page-title break-words"
									>{page.data.ranking.title}</span
								>
								<ChevronDownIcon
									class="size-4 shrink-0 text-primary"
									aria-hidden="true"
								/>
							</button>
						</h1>
						<p class="text-sm text-muted-foreground">
							{page.data.ranking.description}
						</p>
						<Button
							variant="outline"
							class="mt-2 self-start"
							href={`/hotlaps/${data.era.id}/rankings/${page.data.ranking.id}/combinations`}
						>
							<CircleHelpIcon />Rank Info
						</Button>
					</div>
					{#snippet highlight()}
						{#if driverLeaders.length || nationLeaders.length}
							<div class="flex min-w-0 items-stretch">
								{#if driverLeaders.length}
									<RecordHighlight
										title="Leading driver"
										variant="purple"
										class="flex-1 px-10 lg:w-72 lg:flex-none lg:px-14"
									>
										<p
											class="text-right font-mono text-sm font-bold tabular-nums sm:text-lg xl:text-2xl"
											aria-label="Handicap"
										>
											{delta(driverLeaders[0].handicap_ms)}
										</p>
										<div class="grid gap-1 text-sm">
											{#each driverLeaders as leader (leader.player.id)}
												<a
													class="flex min-w-0 items-center justify-end gap-2 font-medium hover:underline"
													href={`/drivers/${encodeURIComponent(leader.player.lfs_username)}`}
												>
													<Flag
														code={leader.player.flag_code}
														fallback={leader.player.country_code}
													/>
													<span class="break-words"
														>{leader.player.display_name}</span
													>
												</a>
											{/each}
										</div>
									</RecordHighlight>
								{/if}
								{#if nationLeaders.length}
									<RecordHighlight
										title="Leading nation"
										variant="yellow"
										class={'flex-1 px-10 lg:w-72 lg:flex-none lg:px-14 ' +
											(driverLeaders.length ? '-ml-[var(--highlight-slant)]' : '')}
									>
										<p
											class="text-right text-lg font-bold tabular-nums xl:text-2xl"
										>
											{nationLeaders[0].points.toLocaleString()}
											<span class="text-sm font-normal">points</span>
										</p>
										<div class="grid gap-1 text-sm">
											{#each nationLeaders as leader (leader.country_code)}
												<p
													class="flex items-center justify-end gap-2 font-medium"
												>
													<Flag
														code={leader.country_code}
													/>{leader.country_code}
												</p>
											{/each}
										</div>
									</RecordHighlight>
								{/if}
							</div>
						{/if}
					{/snippet}
				</PageIdentity>
			{:else}
				<CombinationCard
					track={page.data.track}
					vehicle={page.data.vehicle}
					recordTime={chartRecord}
				/>
			{/if}
			<div class="mx-auto w-full max-w-screen-2xl p-4 md:p-6">
				{@render children()}
			</div>
		{:else}
			<div class="mx-auto w-full max-w-screen-2xl space-y-6 p-4 md:p-6">
				{@render eraContext()}
				{@render children()}
			</div>
		{/if}
	</main>
</CombinationProvider>

<RankingPicker
	bind:this={rankingPicker}
	era={data.era}
	selected={onRanking ? page.data.ranking.id : undefined}
/>
