<script lang="ts">
	import { navigating } from '$app/state';
	import { goto } from '$app/navigation';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import TeamForm from '#lib/components/app/TeamForm.svelte';
	import TeamCard from '#lib/components/app/TeamCard.svelte';
	import type { Team } from '#lib/teams.js';
	import Empty from '#lib/components/app/Empty.svelte';
	import FilterBar from '#lib/components/app/FilterBar.svelte';
	import PaginationControls from '#lib/components/app/PaginationControls.svelte';
	import { setQuery } from '#lib/query.js';
	import type { FilterDefinition } from '#lib/filter-bar.js';
	import { useSession } from '#lib/session.svelte.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const session = useSession();
	let formOpen = $state(false);
	let editingTeam = $state<Team | undefined>(undefined);
	function edit(team: Team) {
		editingTeam = team;
		formOpen = true;
	}
	async function saved(team: Team) {
		const query = new URLSearchParams({
			q: team.name,
			status: team.is_active ? 'active' : 'archived',
		});
		await goto(`/teams?${query}#team-${team.id}`, { invalidateAll: true });
		formOpen = false;
	}
	const loading = $derived(navigating.to?.url.pathname === '/teams');
	const hasFilters = $derived(Boolean(data.filters.q || data.filters.status));
	const definitions: FilterDefinition[] = [
		{ key: 'q', label: 'Name', placeholder: 'Team name...' },
		{
			key: 'status',
			label: 'Status',
			options: [
				{ value: 'active', label: 'Active' },
				{ value: 'archived', label: 'Disbanded' },
				{ value: 'all', label: 'All teams' },
			],
		},
	];
</script>

<Dialog.Root bind:open={formOpen}>
	<main id="main" class="w-full min-w-0 flex-1">
		<PageIdentity label="Team directory identity">
			{#snippet actions()}
				{#if session.signedIn}<Dialog.Trigger
						onclick={() => {
							editingTeam = undefined;
						}}
						>{#snippet child({ props })}<Button {...props}
								><PlusIcon /> Create a team</Button
							>{/snippet}</Dialog.Trigger
					>
				{:else}<Button href="/auth/lfs?return_to=%2Fteams"
						><PlusIcon /> Create a team</Button
					>{/if}
			{/snippet}
			<div class="py-2"><PageHeading title="Teams" /></div>
		</PageIdentity>
		<div class="mx-auto w-full max-w-screen-2xl space-y-4 p-4 md:p-6">
			<FilterBar
				filters={data.filters}
				{loading}
				{definitions}
				label="Team filters"
				onApply={(values) =>
					setQuery({ ...values, page: '', include_archived: '' })}
			/>
			<section
				id="team-results"
				aria-label="Teams"
				class="space-y-3"
				aria-busy={loading}
			>
				{#if loading}<p role="status" class="text-sm text-muted-foreground">
						Loading teams...
					</p>{/if}
				{#if data.teams.items.length}
					<ul class="grid items-start gap-4 lg:grid-cols-2" aria-label="Teams">
						{#each data.teams.items as team (team.id)}{#key team}<TeamCard
									{team}
									onEdit={edit}
								/>{/key}{/each}
					</ul>
				{:else if data.teams.pagination.total_items > 0}<Empty
						title="No teams on this page"
						><p>Choose an earlier page to see teams.</p></Empty
					>
				{:else if hasFilters}<Empty title="No matching teams"
						><p>Try changing the name or removing a filter.</p></Empty
					>
				{:else}<Empty title="No teams available"
						><p>Create a team and invite your friends.</p></Empty
					>{/if}
			</section>
			<PaginationControls pagination={data.teams.pagination} label="teams" />
		</div>
	</main>
	{#if session.signedIn}<Dialog.Content
			class="max-h-[calc(100dvh-4rem)] overflow-y-auto sm:max-w-lg"
		>
			<Dialog.Header
				><Dialog.Title
					>{editingTeam ? 'Edit team' : 'Create a team'}</Dialog.Title
				><Dialog.Description
					>{editingTeam
						? 'Update the team details.'
						: 'Create a team and invite members with a shared link.'}</Dialog.Description
				></Dialog.Header
			>
			{#key editingTeam?.id ?? 'create'}<TeamForm
					team={editingTeam}
					onSaved={saved}
					onCancel={() => {
						formOpen = false;
					}}
				/>{/key}
		</Dialog.Content>{/if}
</Dialog.Root>
