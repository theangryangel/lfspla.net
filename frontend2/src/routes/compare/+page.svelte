<script lang="ts">
	import ComparisonForm from '$lib/components/app/ComparisonForm.svelte';
	import PageHeading from '$lib/components/app/PageHeading.svelte';
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import Empty from '$lib/components/app/Empty.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import { useComparison } from '$lib/compare.svelte.js';
	import type { PlayerChartResultResponse } from '$lib/api.js';
	import { lapTime, delta } from '$lib/format.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const selection = useComparison();
	$effect(() => {
		if (data.comparison)
			selection.seed([
				data.comparison.left.player,
				data.comparison.right.player,
			]);
	});
	const rows = $derived.by(() => {
		const charts = new Map<
			string,
			{
				era: string;
				track: string;
				vehicle: string;
				left?: PlayerChartResultResponse;
				right?: PlayerChartResultResponse;
			}
		>();
		for (const side of ['left', 'right'] as const)
			for (const result of data.comparison?.[side].results ?? []) {
				const key = `${result.era_id}/${result.track}/${result.vehicle}`;
				const row = charts.get(key) ?? {
					era: result.era_id,
					track: result.track,
					vehicle: result.vehicle,
				};
				row[side] = result;
				charts.set(key, row);
			}
		return [...charts.values()].sort(
			(a, b) =>
				a.track.localeCompare(b.track) || a.vehicle.localeCompare(b.vehicle),
		);
	});
	const shared = $derived(rows.filter((row) => row.left && row.right));
	const leftWins = $derived(
		shared.filter((row) => row.left!.lap_time_ms < row.right!.lap_time_ms)
			.length,
	);
	const rightWins = $derived(
		shared.filter((row) => row.right!.lap_time_ms < row.left!.lap_time_ms)
			.length,
	);
</script>

<svelte:head><title>Compare drivers · lfspla.net</title></svelte:head>
<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<PageHeading title="Compare drivers" />
	<ComparisonForm eras={data.eras} initial={data} />
	{#if data.problem}
		<p
			role="alert"
			class="rounded-md border border-destructive p-4 text-destructive"
		>
			{data.problem}
		</p>
	{:else if data.comparison}
		<p class="text-sm text-muted-foreground">
			{shared.length} shared charts · {data.comparison.left.player
				.display_name}: {leftWins} wins · {data.comparison.right.player
				.display_name}: {rightWins} wins · {shared.length -
				leftWins -
				rightWins} ties
		</p>
		{#if rows.length}
			<Card.Root class="gap-0 py-0"
				><Card.Content class="px-0">
					<Table.Root>
						<Table.Caption class="p-4"
							>Current personal bests. Differences compare laps on the same
							chart; unopposed laps do not count as wins.</Table.Caption
						>
						<Table.Header
							><Table.Row
								><Table.Head class="pl-4">Track / Vehicle</Table.Head>
								{#each [data.comparison.left.player, data.comparison.right.player] as player}
									<Table.Head class="text-right"
										><Flag code={player.country_code} class="mr-1" />
										<a
											class="hover:underline"
											href={`/drivers/${encodeURIComponent(player.lfs_username)}`}
											>{player.display_name}</a
										></Table.Head
									>
								{/each}
							</Table.Row></Table.Header
						>
						<Table.Body>
							{#each rows as row (`${row.era}/${row.track}/${row.vehicle}`)}
								<Table.Row>
									<Table.Cell class="pl-4"
										><a
											class="hover:underline"
											href={`/hotlaps/${encodeURIComponent(row.era)}/charts/${encodeURIComponent(row.track)}/${encodeURIComponent(row.vehicle)}`}
											>{row.track} / {row.vehicle}</a
										></Table.Cell
									>
									{#each ['left', 'right'] as side}
										{@const result = side === 'left' ? row.left : row.right}
										{@const other = side === 'left' ? row.right : row.left}
										{@const difference =
											result && other
												? result.lap_time_ms - other.lap_time_ms
												: null}
										<Table.Cell class="text-right font-mono tabular-nums">
											<span
												class:text-time-best={difference !== null &&
													difference < 0}>{lapTime(result?.lap_time_ms)}</span
											>
											{#if result}<span
													class="block text-xs text-muted-foreground"
													class:text-time-best={difference !== null &&
														difference < 0}
													>{difference === null
														? 'Unopposed'
														: difference === 0
															? 'Tie'
															: delta(difference)}</span
												>{/if}
										</Table.Cell>
									{/each}
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</Card.Content></Card.Root
			>
		{:else}<Empty title="No matching laps"
				><p>
					Neither driver has a personal best for these filters. Try another era
					or clear the track and vehicle.
				</p></Empty
			>{/if}
	{:else}
		<Empty title="Choose two drivers"
			><p>
				Add drivers from rankings, timing tables or profiles, or enter two LFS
				usernames above.
			</p></Empty
		>
	{/if}
</main>
