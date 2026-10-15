<script lang="ts">
	import { navigating, page } from '$app/state';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import Breadcrumbs from '$lib/components/app/Breadcrumbs.svelte';
	import SearchIcon from '@lucide/svelte/icons/search';
	import Empty from '$lib/components/app/Empty.svelte';
	import TrackCard from '$lib/components/app/TrackCard.svelte';
	import Thumbnail from '$lib/components/app/Thumbnail.svelte';
	import CornerRibbon from '$lib/components/app/CornerRibbon.svelte';
	import PageIdentity from '$lib/components/app/PageIdentity.svelte';
	import PageHeading from '$lib/components/app/PageHeading.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import { setQuery } from '$lib/query.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let searchError = $state('');
	const loading = $derived(navigating.to?.url.pathname === page.url.pathname);

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
	<div class="mx-auto w-full max-w-screen-2xl px-4 py-4 md:px-6 md:py-3">
		<Breadcrumbs items={data.breadcrumbs} />
	</div>
	<PageIdentity label="Track location identity">
		{#snippet overlay()}<CornerRibbon />{/snippet}
		<div class="flex flex-col gap-4 py-2 sm:flex-row sm:items-center sm:gap-6">
			<Thumbnail
				src={data.location.image?.url}
				class="w-full shrink-0 rounded-md sm:w-64"
			/>
			<div class="min-w-0 space-y-3">
				<PageHeading title={data.location.name}
					><span class="font-mono">{data.location.code}</span></PageHeading
				>
				{#if data.location.description}<p
						class="max-w-prose whitespace-pre-line text-sm text-muted-foreground"
					>
						{data.location.description}
					</p>{/if}
			</div>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-4 p-4 md:p-6">
		<div class="flex flex-wrap items-center justify-between gap-4">
			<h2 class="racing-section-title">Configurations</h2>
			<form
				action={`/tracks/${encodeURIComponent(data.location.code)}`}
				method="GET"
				role="search"
				class="flex w-full gap-2 sm:w-96"
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
						aria-label="Search configurations"
						aria-controls="track-results"
						placeholder="Search by name or code..."
					/>
				</div>
				<Button type="submit" class="h-10" disabled={loading}>Search</Button>
			</form>
		</div>
		{#if searchError}<p role="alert" class="text-destructive">
				{searchError}
			</p>{/if}
		{#if loading}<p role="status" class="text-sm text-muted-foreground">
				Loading tracks...
			</p>{/if}
		<section
			id="track-results"
			aria-label="Track configurations"
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
				<Empty title="No matching configurations"
					><p>No configuration matches “{data.search}”.</p></Empty
				>
			{:else}
				<Empty title="No configurations available" />
			{/if}
		</section>
		<PaginationControls
			pagination={data.tracks.pagination}
			label="configurations"
		/>
	</div>
</main>
