<script lang="ts">
	import { onMount } from 'svelte';
	import Thumbnail from '$lib/components/app/Thumbnail.svelte';
	import { lapTime } from '$lib/format.js';
	import HotlapActivity from '$lib/components/app/HotlapActivity.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import PageIdentity from '$lib/components/app/PageIdentity.svelte';
	import RecordHighlight from '$lib/components/app/RecordHighlight.svelte';
	import PanelCarousel from '$lib/components/app/PanelCarousel.svelte';
	import CommunityCard from '$lib/components/app/CommunityCard.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const communitySlides = $derived(
		data.communityCards?.map(({ id, title }) => ({ id, title })) ?? [],
	);
	let autoplayTurn = $state(0);
	onMount(() => {
		// Each pane advances every 8 seconds, with their turns offset by 4 seconds.
		const timer = window.setInterval(() => {
			if (!document.hidden) autoplayTurn += 1;
		}, 4000);
		return () => window.clearInterval(timer);
	});
	const stats = $derived([
		{ label: 'Eras', value: data.stats?.eras, variant: 'neutral' as const },
		{
			label: 'Hotlaps',
			value: data.stats?.validated_hotlaps,
			variant: 'cyan' as const,
		},
		{
			label: 'Drivers',
			value: data.stats?.drivers,
			variant: 'purple' as const,
		},
		{
			label: 'Combinations',
			value: data.stats?.combinations,
			variant: 'yellow' as const,
		},
	]);
	const combo = $derived(data.stats?.combo_spotlight);
	const driver = $derived(data.stats?.driver_spotlight);
	const target = $derived(
		combo?.leaders.reduce<(typeof combo.leaders)[number] | null>(
			(best, entry) =>
				!best || entry.lap_time_ms < best.lap_time_ms ? entry : best,
			null,
		) ?? null,
	);
	const comboHref = $derived(
		combo
			? `/hotlaps/${encodeURIComponent(combo.chart.era_id)}/charts/${encodeURIComponent(combo.chart.track.code)}/${encodeURIComponent(combo.chart.vehicle.code)}`
			: '',
	);
</script>

<PageIdentity label="Community statistics">
	<div class="space-y-3 py-2">
		<h1 class="racing-page-title">A new home for Live for Speed data.</h1>
		<p class="max-w-prose text-muted-foreground">
			A community-built alternative to LFS World-starting with validated hotlaps
			and (hopefully) growing to include online results, personal bests,
			statistics, and driver history.
		</p>
	</div>
	{#snippet highlight()}
		<div
			class="home-highlights grid min-w-0 grid-cols-2 gap-2 lg:w-[34rem] lg:gap-x-0 xl:w-[48rem] xl:grid-cols-4"
		>
			{#each stats as stat (stat.label)}
				<RecordHighlight
					title={stat.label}
					variant={stat.variant}
					class="w-auto px-8 lg:w-auto lg:px-10"
				>
					<p class="text-right text-2xl font-bold tabular-nums sm:text-3xl">
						{stat.value == null ? '-' : stat.value.toLocaleString()}
					</p>
				</RecordHighlight>
			{/each}
		</div>
	{/snippet}
</PageIdentity>

{#snippet combinationSpotlight()}
	{#if combo}
		<div
			class="grid min-h-30 items-stretch bg-card xl:grid-cols-[minmax(0,1fr)_13rem]"
		>
			<a
				href={comboHref}
				class="grid min-w-0 items-center gap-4 px-5 py-5 sm:grid-cols-[auto_minmax(0,1fr)_minmax(0,1fr)] focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-[-2px]"
			>
				<div class="flex min-w-0 items-center sm:max-w-24">
					<span class="racing-section-title break-words"
						>{combo.chart.era_title}</span
					>
				</div>
				<div class="flex min-w-0 items-center gap-2">
					<Thumbnail
						src={combo.chart.track.image?.url ?? combo.chart.track.outline?.url}
						class="w-14 shrink-0 bg-transparent"
					/>
					<div class="min-w-0">
						<h3 class="racing-section-title break-words">
							{combo.chart.track.name}
						</h3>
						<p class="mt-1 text-sm text-muted-foreground">
							{combo.chart.track.code}
						</p>
					</div>
				</div>
				<div class="flex min-w-0 items-center gap-2">
					<Thumbnail
						src={combo.chart.vehicle.image?.url}
						class="w-14 shrink-0 bg-transparent"
					/>
					<div class="min-w-0">
						<h3 class="racing-section-title break-words">
							{combo.chart.vehicle.name}
						</h3>
						<p class="mt-1 text-sm text-muted-foreground">
							{combo.chart.vehicle.code}
						</p>
					</div>
				</div>
			</a>
			<RecordHighlight
				title="World Record"
				variant="purple"
				compact
				flushEnd
				class="content-center py-4 text-right lg:w-auto lg:py-4"
			>
				<p class="text-right font-mono text-2xl font-bold tabular-nums">
					{target
						? lapTime(target.lap_time_ms - target.distance_to_world_record_ms)
						: '-'}
				</p>
			</RecordHighlight>
		</div>
	{:else}
		<p class="min-h-30 bg-card p-5 text-sm text-muted-foreground">
			{data.stats ? 'No validated uploads yet.' : 'Spotlight unavailable.'}
		</p>
	{/if}
{/snippet}

{#snippet driverSpotlight()}
	{#if driver}
		<div
			class="grid min-h-30 items-stretch bg-card grid-cols-[minmax(0,2fr)_minmax(9rem,1fr)]"
		>
			<a
				class="flex min-w-0 items-center gap-3 px-5 py-5 hover:text-primary focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-[-2px]"
				href={`/drivers/${encodeURIComponent(driver.player.lfs_username)}`}
			>
				<Flag
					code={driver.player.flag_code}
					fallback={driver.player.country_code}
				/>
				<span class="min-w-0 break-words text-xl font-semibold"
					>{driver.player.display_name}</span
				>
			</a>
			<RecordHighlight
				title="Recent PBs"
				variant="cyan"
				compact
				flushEnd
				class="content-center py-4 pl-10 pr-5 text-right lg:w-auto lg:py-4"
			>
				<p class="text-right text-3xl font-bold tabular-nums">
					{driver.recent_personal_bests.toLocaleString()}
				</p>
			</RecordHighlight>
		</div>
	{:else}
		<p class="bg-card p-5 text-sm text-muted-foreground">
			{data.stats
				? 'No current personal bests among recent uploads.'
				: 'Spotlight unavailable.'}
		</p>
	{/if}
{/snippet}

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<div
		class={[
			'grid min-w-0 items-stretch gap-6',
			communitySlides.length > 0 && 'lg:grid-cols-2',
		]}
	>
		<PanelCarousel
			id="home-spotlight"
			autoplayStep={Math.floor(autoplayTurn / 2)}
			label="Community spotlights"
			variant="purple"
			slides={[
				{ title: 'Combination spotlight', content: combinationSpotlight },
				{ title: 'Driver spotlight', content: driverSpotlight },
			]}
		/>
		{#if data.communityCards?.length}
			<PanelCarousel
				id="home-community"
				autoplayStep={Math.floor(Math.max(0, autoplayTurn - 1) / 2)}
				label="Community & partners"
				title="Community & partners"
				variant="cyan"
				slides={communitySlides}
			>
				{#snippet slideContent(index)}
					{#if data.communityCards?.[index]}
						<CommunityCard card={data.communityCards[index]} />
					{/if}
				{/snippet}
			</PanelCarousel>
		{/if}
	</div>
	<HotlapActivity
		title="Activity"
		appearance="data"
		refreshKey={data.stats}
		views={['all', 'world_records', 'mine']}
		perPage={10}
		eras={data.eras}
	/>
</main>

<style>
	@media (min-width: 64rem) {
		.home-highlights {
			/* Include the same overlap in every panel, including the first in a row. */
			padding-left: var(--highlight-slant);
		}
		.home-highlights :global(.record-highlight) {
			margin-left: calc(-1 * var(--highlight-slant));
		}
	}
</style>
