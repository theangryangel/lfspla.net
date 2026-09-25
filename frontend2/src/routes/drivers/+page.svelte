<script lang="ts">
	import { beforeNavigate } from '$app/navigation';
	import { onDestroy } from 'svelte';
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import Empty from '$lib/components/app/Empty.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import Breadcrumbs from '$lib/components/app/Breadcrumbs.svelte';
	import { queryValue, updateQuery } from '$lib/query.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let draft = $state<string | null>(null);
	const search = $derived(draft ?? queryValue('q'));
	const searches = new Set<string>();
	let searchError = $state('');
	let timer: ReturnType<typeof setTimeout> | undefined;
	const cancelSearch = () => clearTimeout(timer);
	beforeNavigate(({ type, to }) => {
		if (
			type === 'goto' &&
			to?.url.pathname === '/drivers' &&
			searches.has(to.url.searchParams.get('q') ?? '')
		)
			return;
		searches.clear();
		cancelSearch();
		draft = null;
	});
	onDestroy(cancelSearch);

	function searchDrivers(value: string) {
		draft = value;
		searchError = '';
		cancelSearch();
		timer = setTimeout(() => {
			// Keep newer typing local while an older search is loading.
			searches.add(value);
			void updateQuery('q', value, true)
				.catch(() => {
					searchError = 'Could not search drivers. Please try again.';
				})
				.finally(() => searches.delete(value));
		}, 200);
	}
</script>

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<Breadcrumbs items={data.breadcrumbs} />
	<div class="flex flex-wrap items-center gap-4">
		<Input
			class="sm:max-w-xs"
			aria-label="Search drivers"
			placeholder="Search drivers..."
			value={search}
			oninput={(e) => searchDrivers(e.currentTarget.value)}
		/>
		{#if data.players}
			<span class="text-sm text-muted-foreground sm:ml-auto">
				{data.players.length}
				{data.players.length === 1 ? 'driver' : 'drivers'}
			</span>
		{/if}
	</div>
	{#if searchError}<p role="alert" class="text-destructive">
			{searchError}
		</p>{/if}
	{#if !data.players}
		<Empty title="Search for a driver">
			<p>Type a username or display name to search the directory.</p>
		</Empty>
	{:else if data.players.length}
		<Card.Root class="gap-0 py-0">
			<Card.Content
				class="px-0 [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
			>
				<Table.Root>
					<Table.Caption>The first 25 matches, in username order.</Table.Caption
					>
					<Table.Header>
						<Table.Row>
							<Table.Head>Driver</Table.Head>
							<Table.Head>LFS username</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each data.players as player (player.id)}
							<Table.Row>
								<Table.Cell class="font-medium">
									<div class="flex min-w-0 items-center gap-1">
										<a
											class="min-w-0 truncate hover:underline"
											href="/drivers/{encodeURIComponent(player.lfs_username)}"
										>
											{#if player.country_code}
												<Flag code={player.country_code} />
											{:else}
												<span class="text-muted-foreground">-</span>
											{/if}
											{player.display_name}
										</a>
										<CompareButton driver={player} />
									</div>
								</Table.Cell>
								<Table.Cell class="text-muted-foreground"
									>{player.lfs_username}</Table.Cell
								>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</Card.Content>
		</Card.Root>
	{:else}
		<Empty title="No matching drivers">
			<p>No driver matches “{data.search}”.</p>
		</Empty>
	{/if}
</main>
