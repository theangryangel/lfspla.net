<script lang="ts">
	import { onMount } from 'svelte';
	import HotlapActivity from '#lib/components/app/HotlapActivity.svelte';
	import SpotlightCard from '#lib/components/app/SpotlightCard.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import RecordHighlight from '#lib/components/app/RecordHighlight.svelte';
	import PanelCarousel from '#lib/components/app/PanelCarousel.svelte';
	import CommunityCard from '#lib/components/app/CommunityCard.svelte';
	import RacingThisWeek from '#lib/components/app/RacingThisWeek.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const communityCards = $derived(data.communityCards);
	const communitySlides = $derived(
		communityCards.map(({ id, title }) => ({ id, title })),
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
	const spotlightSlides = $derived(data.stats?.spotlights ?? []);
	const showEvents = $derived(
		data.upcomingEvents === null || data.upcomingEvents.length > 0,
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
			slides={spotlightSlides.length
				? spotlightSlides.map(({ id, title }) => ({ id, title }))
				: [{ id: 'empty', title: 'Community spotlights' }]}
		>
			{#snippet slideContent(index)}
				{#if spotlightSlides[index]}
					<SpotlightCard card={spotlightSlides[index]} />
				{:else}
					<p class="min-h-30 bg-card p-5 text-sm text-muted-foreground">
						{data.stats
							? 'No spotlights available yet.'
							: 'Spotlights unavailable.'}
					</p>
				{/if}
			{/snippet}
		</PanelCarousel>
		{#if communityCards.length}
			<PanelCarousel
				id="home-community"
				autoplayStep={Math.floor(Math.max(0, autoplayTurn - 1) / 2)}
				label="Community & partners"
				title="Community & partners"
				variant="cyan"
				slides={communitySlides}
			>
				{#snippet slideContent(index)}
					{#if communityCards[index]}
						<CommunityCard card={communityCards[index]} />
					{/if}
				{/snippet}
			</PanelCarousel>
		{/if}
	</div>
	<div
		class={[
			'grid min-w-0 items-start gap-6',
			showEvents && 'xl:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]',
		]}
	>
		<div class="min-w-0">
			<HotlapActivity
				title="Activity"
				appearance="data"
				refreshKey={data.stats}
				views={['all', 'world_records', 'longest_standing', 'mine']}
				perPage={10}
				showContributions={false}
				eras={data.eras}
			/>
		</div>
		{#if showEvents}
			<aside
				class="min-w-0 border-t bg-card/60 px-4 pb-4 pt-4 xl:border-t-0 xl:border-l xl:pt-0"
				aria-label="Racing this week"
			>
				<RacingThisWeek events={data.upcomingEvents} />
			</aside>
		{/if}
	</div>
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
