<script lang="ts">
	import { navigating } from '$app/state';
	import Empty from '#lib/components/app/Empty.svelte';
	import VehicleExplorerCard from '#lib/components/app/VehicleExplorerCard.svelte';
	import VehicleFilterBar from '#lib/components/app/VehicleFilterBar.svelte';
	import CornerRibbon from '#lib/components/app/CornerRibbon.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import PaginationControls from '#lib/components/app/PaginationControls.svelte';
	import { filterKeys } from '#lib/vehicle-explorer.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const loading = $derived(navigating.to?.url.pathname === '/vehicles');
	const hasFilters = $derived(
		filterKeys.some((key) => key !== 'sort' && Boolean(data.filters[key])),
	);
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Vehicle directory identity">
		{#snippet overlay()}<CornerRibbon />{/snippet}
		<div class="py-2">
			<PageHeading title="Vehicles"
				>Explore standard vehicles and vehicle mods.</PageHeading
			>
		</div>
	</PageIdentity>
	<div class="racing-data mx-auto w-full max-w-screen-2xl space-y-4 p-4 md:p-6">
		<VehicleFilterBar filters={data.filters} {loading} />
		<section
			id="vehicle-results"
			aria-label="Vehicle catalogue"
			aria-busy={loading}
		>
			{#if data.vehicles.items.length}
				<ul
					class="grid grid-cols-1 gap-3 min-[560px]:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-6"
				>
					{#each data.vehicles.items as vehicle (vehicle.code)}<li
							class="min-w-0"
						>
							<VehicleExplorerCard {vehicle} />
						</li>{/each}
				</ul>
			{:else if data.vehicles.pagination.total_items > 0}
				<Empty title="No vehicles on this page"
					><p>Choose an earlier page to see vehicles.</p></Empty
				>
			{:else if hasFilters}
				<Empty title="No matching vehicles"
					><p>
						Try widening a specification range or removing a filter.
					</p></Empty
				>
			{:else}<Empty title="No vehicles available" />{/if}
		</section>
		<PaginationControls
			pagination={data.vehicles.pagination}
			label="vehicles"
		/>
	</div>
</main>
