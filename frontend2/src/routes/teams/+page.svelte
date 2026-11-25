<script lang="ts">
	import { navigating } from '$app/state';
	import { useSession } from '#lib/session.svelte.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import Breadcrumbs from '#lib/components/app/Breadcrumbs.svelte';
	import TeamTags from '#lib/components/app/TeamTags.svelte';
	import HelpButton from '#lib/components/app/HelpButton.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import FilterBar from '#lib/components/app/FilterBar.svelte';
	import PaginationControls from '#lib/components/app/PaginationControls.svelte';
	import Empty from '#lib/components/app/Empty.svelte';
	import { setQuery } from '#lib/query.js';
	import { disciplineLabels } from '#lib/teams.js';
	import type { FilterDefinition } from '#lib/filter-bar.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const session = useSession();
	const loading = $derived(navigating.to?.url.pathname === '/teams');
	const hasFilters = $derived(Object.values(data.filters).some(Boolean));
	const definitions: FilterDefinition[] = $derived([
		{ key: 'q', label: 'Name or tag', placeholder: 'Team name or tag...' },
		{
			key: 'discipline',
			label: 'Discipline',
			options: Object.entries(disciplineLabels).map(([value, label]) => ({
				value,
				label,
			})),
		},
		{
			key: 'accepts_applications',
			label: 'Applications',
			options: [
				{ value: 'true', label: 'Open' },
				{ value: 'false', label: 'Closed' },
			],
		},
		...(session.signedIn
			? [
					{
						key: 'mine',
						label: 'My teams',
						options: [{ value: 'true', label: 'Only' }],
					},
				]
			: []),
	]);
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<div
		class="mx-auto flex w-full max-w-screen-2xl flex-wrap items-center justify-between gap-3 px-4 py-4 md:px-6 md:py-3"
	>
		<Breadcrumbs items={data.breadcrumbs} />
		<div class="flex flex-wrap items-center gap-2">
			<HelpButton />
			{#if session.signedIn}<Button href="/teams/new" variant="outline"
					>Create team</Button
				>{/if}
		</div>
	</div>
	<PageIdentity label="Team directory identity">
		<div class="py-2">
			<PageHeading title="Teams"
				>Find a team. Get to know the drivers behind the tag.</PageHeading
			>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-4 p-4 md:p-6">
		<FilterBar
			filters={data.filters}
			{loading}
			{definitions}
			label="Team filters"
			onApply={(values) => setQuery({ ...values, page: '' })}
		/>
		<section aria-label="Team matches" aria-busy={loading} class="space-y-3">
			{#if loading}<p role="status" class="text-sm text-muted-foreground">
					Loading teams...
				</p>{/if}
			{#if data.teams.items.length}
				<TableFrame appearance="data">
					<Table.Root
						class="table-fixed [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4"
						aria-label="Teams"
					>
						<Table.Header>
							<Table.Row>
								<Table.Head>Team</Table.Head>
								<Table.Head class="hidden w-80 sm:table-cell">Tags</Table.Head>
								<Table.Head class="w-20 text-right">Members</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each data.teams.items as team (team.id)}
								<Table.Row>
									<Table.Cell class="min-w-0 whitespace-normal py-3">
										<a
											href="/teams/{team.id}"
											class="group block min-w-0 space-y-1.5 rounded-sm py-1 focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-2"
										>
											<div
												class="flex flex-wrap items-baseline gap-x-3 gap-y-1"
											>
												<span
													class="break-words font-semibold group-hover:underline"
													>{team.name}</span
												>
												{#if team.tag}<span
														class="break-all font-mono text-sm text-muted-foreground"
														>{team.tag}</span
													>{/if}
											</div>
											{#if team.description}<p
													class="line-clamp-1 break-words text-sm text-muted-foreground"
												>
													{team.description}
												</p>{/if}
											<div class="pt-1 sm:hidden"><TeamTags {team} /></div>
										</a>
									</Table.Cell>
									<Table.Cell class="hidden whitespace-normal sm:table-cell"
										><TeamTags {team} /></Table.Cell
									>
									<Table.Cell
										class="text-right tabular-nums text-muted-foreground"
										>{team.member_count.toLocaleString()}</Table.Cell
									>
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</TableFrame>
			{:else if data.teams.pagination.total_items > 0}
				<Empty title="No teams on this page"
					><p>Choose an earlier page to see teams.</p></Empty
				>
			{:else if hasFilters}
				<Empty title="No matching teams"
					><p>Try a different name or remove a filter.</p>
					<a href="/teams" class="text-primary underline underline-offset-4"
						>Clear filters</a
					></Empty
				>
			{:else}
				<Empty title="No teams available" />
			{/if}
		</section>
		<PaginationControls pagination={data.teams.pagination} label="teams" />
	</div>
</main>
