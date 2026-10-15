<script lang="ts">
	import { navigating } from '$app/state';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import SearchIcon from '@lucide/svelte/icons/search';
	import Empty from '$lib/components/app/Empty.svelte';
	import TrackCard from '$lib/components/app/TrackCard.svelte';
	import CornerRibbon from '$lib/components/app/CornerRibbon.svelte';
	import PageIdentity from '$lib/components/app/PageIdentity.svelte';
	import PageHeading from '$lib/components/app/PageHeading.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import { setQuery } from '$lib/query.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let searchError = $state('');
	const loading = $derived(navigating.to?.url.pathname === '/tracks');

	async function searchTracks(event: SubmitEvent) {
		event.preventDefault();
		const form = event.currentTarget as HTMLFormElement;
		const search = String(new FormData(form).get('q') ?? '').trim();
		searchError = '';
		try {
			await setQuery({ q: search, page: '' });
		} catch {
			searchError = 'Could not search tracks. Please try again.';
		}
	}
</script>


<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Track directory identity">
		{#snippet overlay()}<CornerRibbon />{/snippet}
		<div
			class="grid min-w-0 items-center gap-4 py-2 md:grid-cols-[minmax(0,1fr)_24rem] md:gap-8"
		>
			<PageHeading title="Tracks"
				>Explore LFS tracks and their configurations.</PageHeading
			>
			<form
				action="/tracks"
				method="GET"
				role="search"
				class="flex min-w-0 gap-2"
				onsubmit={searchTracks}
			>
				<div class="relative min-w-0 flex-1">
					<SearchIcon
						class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
						aria-hidden="true"
					/>
					<Input
						name="q"
						value={data.search}
						class="h-10 pl-10"
						aria-label="Search tracks"
						aria-controls="track-results"
						placeholder="Search by name or code..."
					/>
				</div>
				<Button type="submit" class="h-10" disabled={loading}>Search</Button>
			</form>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-4 p-4 md:p-6">
		{#if searchError}<p role="alert" class="text-destructive">
				{searchError}
			</p>{/if}
		{#if loading}<p role="status" class="text-sm text-muted-foreground">
				Loading tracks...
			</p>{/if}
		<section
			id="track-results"
			aria-label="Track catalogue"
			aria-busy={loading}
		>
			{#if data.tracks.items.length}
				<ul
					class="grid grid-cols-1 gap-2 text-sm min-[400px]:grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7"
				>
					{#each data.tracks.items as track (track.code)}
						<li class="min-w-0"><TrackCard {track} /></li>
					{/each}
				</ul>
			{:else if data.tracks.pagination.total_items > 0}
				<Empty title="No tracks on this page"
					><p>Choose an earlier page to see tracks.</p></Empty
				>
			{:else if data.search}
				<Empty title="No matching tracks"
					><p>No track matches “{data.search}”.</p></Empty
				>
			{:else}
				<Empty title="No tracks available" />
			{/if}
		</section>
		<PaginationControls pagination={data.tracks.pagination} label="tracks" />
	</div>
</main>
