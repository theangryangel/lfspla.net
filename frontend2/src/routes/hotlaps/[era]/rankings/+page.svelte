<script lang="ts">
	import { page } from '$app/state';
	import { Input } from '#lib/components/ui/input/index.js';
	import HotlapDisciplineBadge from '#lib/components/app/HotlapDisciplineBadge.svelte';
	import Progress from '#lib/components/app/Progress.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import Empty from '#lib/components/app/Empty.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let search = $state(page.url.searchParams.get('q') ?? '');
	const matches = $derived(
		data.rankings.filter((rank) =>
			`${rank.title} ${rank.description}`
				.toLowerCase()
				.includes(search.trim().toLowerCase()),
		),
	);
	const progress = $derived(
		Object.fromEntries(
			data.rankings
				.filter((rank) => rank.my_progress)
				.map((rank) => [
					rank.id,
					{
						completed: rank.my_progress!.completed_charts,
						total: rank.my_progress!.total_charts,
					},
				]),
		),
	);
</script>

<div class="space-y-6">
	<Input
		type="search"
		bind:value={search}
		aria-label="Search rankings"
		placeholder="Search rankings…"
		class="max-w-md"
	/>
	{#if matches.length}
		<TableFrame appearance="data">
			<Table.Root class="[&_th]:px-5 [&_th]:py-3 [&_td]:px-5 [&_td]:py-4">
				<Table.Caption class="sr-only">Rankings</Table.Caption>
				<Table.Header>
					<Table.Row>
						<Table.Head scope="col">Ranking</Table.Head>
						{#if Object.keys(progress).length}
							<Table.Head scope="col" class="w-56">
								<span class="block">Your progress</span>
							</Table.Head>
						{/if}
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each matches as ranking (ranking.id)}
						<Table.Row>
							<Table.Cell class="whitespace-normal">
								<div class="py-2">
									<div class="flex flex-wrap items-center gap-2">
										<a
											href={`/hotlaps/${data.era.id}/rankings/${encodeURIComponent(ranking.id)}`}
											class="font-semibold hover:underline focus-visible:outline-2 focus-visible:outline-ring"
											>{ranking.title}</a
										>
										<HotlapDisciplineBadge discipline={ranking.discipline} />
									</div>
									{#if ranking.description}<p
											class="mt-1 text-sm text-muted-foreground"
										>
											{ranking.description}
										</p>{/if}
								</div>
							</Table.Cell>
							{#if Object.keys(progress).length}
								<Table.Cell>
									{#if progress[ranking.id]}
										{@const own = progress[ranking.id]}
										<a
											href={`/hotlaps/${data.era.id}/rankings/${encodeURIComponent(ranking.id)}/combinations`}
											aria-label={`${ranking.title}: ${own.completed} of ${own.total} combinations completed`}
											class="block focus-visible:outline-2 focus-visible:outline-ring"
										>
											<Progress value={own.completed} total={own.total} />
										</a>
									{:else}<span class="text-muted-foreground">—</span>{/if}
								</Table.Cell>
							{/if}
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		</TableFrame>
	{:else}
		<Empty
			title={search.trim()
				? 'No matching rankings'
				: 'No rankings in this era yet.'}
		/>
	{/if}
</div>
