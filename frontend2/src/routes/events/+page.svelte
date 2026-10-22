<script lang="ts">
	import { navigating } from '$app/state';
	import Empty from '#lib/components/app/Empty.svelte';
	import CornerRibbon from '#lib/components/app/CornerRibbon.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import PaginationControls from '#lib/components/app/PaginationControls.svelte';
	import EventCard from '#lib/components/app/EventCard.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const loading = $derived(navigating.to?.url.pathname === '/events');
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
			class="space-y-2 border-l-2 border-primary bg-muted/50 px-4 py-3 text-sm text-muted-foreground"
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
				class="flex flex-wrap items-center gap-3 border-l-2 border-primary bg-muted/50 px-4 py-3 text-sm"
			>
				<p>Showing events for the selected league or season.</p>
				<a class="text-primary underline underline-offset-4" href="/events"
					>Show all events</a
				>
			</div>
		{/if}
		{#if loading}<p role="status" class="text-sm text-muted-foreground">
				Loading events...
			</p>{/if}
		<section aria-label="Upcoming events" aria-busy={loading}>
			{#if data.events.items.length}
				<ul class="space-y-3">
					{#each data.events.items as event (event.id)}
						<li><EventCard {event} /></li>
					{/each}
				</ul>
			{:else if data.events.pagination.total_items > 0}
				<Empty title="No events on this page"
					><p>Choose an earlier page to see upcoming events.</p></Empty
				>
			{:else}
				<Empty title="No upcoming events"
					><p>
						Check back soon, or visit the <a
							class="text-primary underline underline-offset-4"
							href="https://www.lfs.net/leagues/calendar">LFS event calendar</a
						>.
					</p></Empty
				>
			{/if}
		</section>
		<PaginationControls pagination={data.events.pagination} label="events" />
	</div>
</main>
