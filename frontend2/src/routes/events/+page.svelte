<script lang="ts">
	import CornerRibbon from '#lib/components/app/CornerRibbon.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import EventsCalendar from '#lib/components/app/EventsCalendar.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<svelte:head><title>Events | lfspla.net</title></svelte:head>

<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Events identity">
		{#snippet overlay()}<CornerRibbon />{/snippet}
		<div class="py-2">
			<PageHeading title="Events"
				>Find your next Live for Speed race.</PageHeading
			>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-4 p-4 md:p-6">
		<aside
			aria-label="More event resources"
			class="space-y-2 border-l-2 border-primary bg-muted/50 px-4 py-3 text-muted-foreground"
		>
			<p>
				Find league schedules and event details on the <a
					class="text-primary underline underline-offset-4"
					href="https://www.lfs.net/leagues/calendar"
					target="_blank"
					rel="noopener noreferrer">LFS event calendar</a
				>.
			</p>
		</aside>
		{#if data.filtered}
			<div
				class="flex flex-wrap items-center gap-3 border-l-2 border-primary bg-muted/50 px-4 py-3"
			>
				<p>Showing events for the selected league or season.</p>
				<a class="text-primary underline underline-offset-4" href="/events"
					>Show all events</a
				>
			</div>
		{/if}
		{#key `${data.leagueId ?? ''}/${data.seasonId ?? ''}`}
			<EventsCalendar leagueId={data.leagueId} seasonId={data.seasonId} />
		{/key}
	</div>
</main>
