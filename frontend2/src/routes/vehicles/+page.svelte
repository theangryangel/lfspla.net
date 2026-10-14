<script lang="ts">
	import { navigating } from '$app/state';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import SearchIcon from '@lucide/svelte/icons/search';
	import Empty from '$lib/components/app/Empty.svelte';
	import VehicleCard from '$lib/components/app/VehicleCard.svelte';
	import CornerRibbon from '$lib/components/app/CornerRibbon.svelte';
	import PageIdentity from '$lib/components/app/PageIdentity.svelte';
	import PageHeading from '$lib/components/app/PageHeading.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import { setQuery } from '$lib/query.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let searchError = $state('');
	const loading = $derived(navigating.to?.url.pathname === '/vehicles');

	async function searchVehicles(event: SubmitEvent) {
		event.preventDefault();
		const form = event.currentTarget as HTMLFormElement;
		const search = String(new FormData(form).get('q') ?? '').trim();
		searchError = '';
		try {
			await setQuery({ q: search, page: '' });
		} catch {
			searchError = 'Could not search vehicles. Please try again.';
		}
	}
</script>

<svelte:head><title>Vehicles | LFS Planet</title></svelte:head>

<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Vehicle directory identity">
		{#snippet overlay()}<CornerRibbon />{/snippet}
		<div
			class="grid min-w-0 items-center gap-4 py-2 md:grid-cols-[minmax(0,1fr)_24rem] md:gap-8"
		>
			<PageHeading title="Vehicles"
				>Explore standard vehicles and vehicle mods.</PageHeading
			>
			<form
				action="/vehicles"
				method="GET"
				role="search"
				class="flex min-w-0 gap-2"
				onsubmit={searchVehicles}
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
						aria-label="Search vehicles"
						aria-controls="vehicle-results"
						placeholder="Search by name or code…"
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
				Loading vehicles…
			</p>{/if}
		<section
			id="vehicle-results"
			aria-label="Vehicle catalogue"
			aria-busy={loading}
		>
			{#if data.vehicles.items.length}
				<ul
					class="grid grid-cols-1 gap-2 text-sm min-[400px]:grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7"
				>
					{#each data.vehicles.items as vehicle (vehicle.code)}
						<li class="min-w-0"><VehicleCard {vehicle} /></li>
					{/each}
				</ul>
			{:else if data.vehicles.pagination.total_items > 0}
				<Empty title="No vehicles on this page"
					><p>Choose an earlier page to see vehicles.</p></Empty
				>
			{:else if data.search}
				<Empty title="No matching vehicles"
					><p>No vehicle matches “{data.search}”.</p></Empty
				>
			{:else}
				<Empty title="No vehicles available" />
			{/if}
		</section>
		<PaginationControls
			pagination={data.vehicles.pagination}
			label="vehicles"
		/>
	</div>
</main>
