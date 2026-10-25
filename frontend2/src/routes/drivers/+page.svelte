<script lang="ts">
	import { navigating } from '$app/state';
	import CompareButton from '#lib/components/app/CompareButton.svelte';
	import Empty from '#lib/components/app/Empty.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import { setQuery } from '#lib/query.js';
	import FilterBar from '#lib/components/app/FilterBar.svelte';
	import PaginationControls from '#lib/components/app/PaginationControls.svelte';
	import type { FilterDefinition } from '#lib/filter-bar.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const loading = $derived(navigating.to?.url.pathname === '/drivers');
	const hasFilters = $derived(Boolean(data.filters.q || data.filters.country));
	const definitions: FilterDefinition[] = $derived([
		{ key: 'q', label: 'Name', placeholder: 'Username or display name…' },
		{
			key: 'country',
			label: 'Country',
			options: data.countries.map((country) => ({
				value: country.code,
				label: country.name,
			})),
		},
	]);
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Driver directory identity">
		<div class="py-2">
			<PageHeading title="Drivers"
				>Find drivers and explore their racing records.</PageHeading
			>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-4 p-4 md:p-6">
		<FilterBar
			filters={data.filters}
			{loading}
			{definitions}
			label="Driver filters"
			onApply={(values) => setQuery({ ...values, page: '' })}
		/>
		<section
			id="driver-results"
			aria-label="Driver matches"
			class="w-full space-y-3"
			aria-busy={loading}
		>
			{#if loading}<p role="status" class="text-sm text-muted-foreground">
					Loading drivers…
				</p>{/if}
			{#if data.players.items.length}
				<ul aria-busy={loading} class="divide-y border-y text-sm">
					{#each data.players.items as player (player.id)}
						<li
							class="flex min-w-0 items-center gap-3 pr-4 transition-colors hover:bg-muted/50"
						>
							<a
								class="group flex min-w-0 flex-1 items-center gap-3 px-4 py-3 focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-[-2px]"
								href="/drivers/{encodeURIComponent(player.lfs_username)}"
							>
								{#if player.flag_code || player.country_code}
									<Flag
										code={player.flag_code}
										fallback={player.country_code}
										class="shrink-0"
									/>
								{/if}
								<span class="min-w-0">
									<span
										class="block break-words font-semibold group-hover:underline"
										>{player.display_name}</span
									>
									<span class="block break-words text-muted-foreground"
										>{player.lfs_username}</span
									>
								</span>
							</a>
							<div class="shrink-0"><CompareButton driver={player} /></div>
						</li>
					{/each}
				</ul>
				<p class="text-xs text-muted-foreground">
					Matches are shown in username order.
				</p>
			{:else if data.players.pagination.total_items > 0}
				<Empty title="No drivers on this page"
					><p>Choose an earlier page to see drivers.</p></Empty
				>
			{:else if hasFilters}
				<Empty title="No matching drivers"
					><p>Try changing the name or removing a filter.</p></Empty
				>
			{:else}
				<Empty title="No drivers available" />
			{/if}
		</section>
		<PaginationControls pagination={data.players.pagination} label="drivers" />
	</div>
</main>
