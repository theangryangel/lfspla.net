<script lang="ts">
	import ComparisonForm from '#lib/components/app/ComparisonForm.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import CornerRibbon from '#lib/components/app/CornerRibbon.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import Empty from '#lib/components/app/Empty.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
	import { useComparison } from '#lib/compare.svelte.js';
	import type { Hotlap } from '#lib/api.js';
	import { lapTime, delta } from '#lib/format.js';
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
				left?: Hotlap;
				right?: Hotlap;
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
	const visibleRows = $derived(data.sharedOnly ? shared : rows);
	const leftWins = $derived(
		shared.filter((row) => row.left!.lap_time_ms < row.right!.lap_time_ms)
			.length,
	);
	const rightWins = $derived(
		shared.filter((row) => row.right!.lap_time_ms < row.left!.lap_time_ms)
			.length,
	);
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Driver comparison identity">
		{#snippet overlay()}<CornerRibbon />{/snippet}
		<div class="py-2">
			<PageHeading title="Compare drivers"
				>Compare personal bests across an era, track or vehicle.</PageHeading
			>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-6 p-4 md:p-6">
		<ComparisonForm eras={data.eras} initial={data} />
		{#if data.problem}
			<p
				role="alert"
				class="rounded-md border border-destructive p-4 text-destructive"
			>
				{data.problem}
			</p>
		{:else if data.comparison}
			<section
				aria-label="Comparison summary"
				class="grid grid-cols-2 gap-4 border-y bg-card px-4 py-5 sm:grid-cols-3"
			>
				{#each [{ player: data.comparison.left.player, wins: leftWins }, { player: data.comparison.right.player, wins: rightWins }] as side, index}
					<div
						class={index === 0 ? 'min-w-0 sm:order-0' : 'min-w-0 sm:order-2'}
					>
						<div class="flex min-w-0 items-center gap-2">
							<Flag
								code={side.player.flag_code}
								fallback={side.player.country_code}
								class="shrink-0"
							/>
							<a
								class="min-w-0 break-words font-semibold hover:underline"
								href="/drivers/{encodeURIComponent(side.player.lfs_username)}"
								>{side.player.display_name}</a
							>
						</div>
						<p class="mt-1 break-words text-sm text-muted-foreground">
							{side.player.lfs_username}
						</p>
						<p class="mt-3 flex items-baseline gap-2">
							<span class="text-3xl font-bold tabular-nums"
								>{side.wins.toLocaleString()}</span
							><span class="text-sm text-muted-foreground">wins</span>
						</p>
					</div>
				{/each}
				<div
					class="order-first col-span-2 border-b pb-4 text-center sm:order-1 sm:col-span-1 sm:border-x sm:border-b-0 sm:px-4 sm:pb-0"
				>
					<h2 class="racing-section-title">Head to head</h2>
					<p class="mt-2 text-sm text-muted-foreground">
						<span class="font-semibold tabular-nums text-foreground"
							>{shared.length.toLocaleString()}</span
						> shared charts
					</p>
					<p class="mt-1 text-sm text-muted-foreground">
						{(shared.length - leftWins - rightWins).toLocaleString()} ties
					</p>
				</div>
			</section>
			{#if visibleRows.length}
				<section class="space-y-3" aria-labelledby="comparison-results-title">
					<h2 id="comparison-results-title" class="racing-section-title">
						Personal bests
					</h2>
					<TableFrame appearance="data">
						<Table.Root
							class="min-w-[32rem] [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4"
						>
							<Table.Caption class="px-4 py-3 text-left"
								>Current personal bests. Differences compare laps on the same
								chart; unopposed laps do not count as wins.</Table.Caption
							>
							<Table.Header
								><Table.Row
									><Table.Head class="pl-4">Track / Vehicle</Table.Head>
									{#each [data.comparison.left.player, data.comparison.right.player] as player}
										<Table.Head class="text-right">
											<span class="inline-flex items-center justify-end gap-2"
												><Flag
													code={player.flag_code}
													fallback={player.country_code}
													class="shrink-0"
												/>
												<a
													class="hover:underline"
													href={`/drivers/${encodeURIComponent(player.lfs_username)}`}
													>{player.display_name}</a
												></span
											></Table.Head
										>
									{/each}
								</Table.Row></Table.Header
							>
							<Table.Body>
								{#each visibleRows as row (`${row.era}/${row.track}/${row.vehicle}`)}
									<Table.Row>
										<Table.Cell class="pl-4"
											><a
												class="font-medium hover:underline"
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
											<Table.Cell
												class="text-right font-mono tabular-nums py-2"
											>
												<span
													class="text-base font-semibold"
													class:text-time-best={difference !== null &&
														difference < 0}>{lapTime(result?.lap_time_ms)}</span
												>
												{#if result}<span
														class="block text-sm text-muted-foreground"
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
					</TableFrame>
				</section>
			{:else}<Empty title="No matching laps"
					><p>
						{data.sharedOnly
							? 'The drivers have no laps on the same chart for these filters. Untick show shared charts only, try another era or clear the track and vehicle.'
							: 'Neither driver has a personal best for these filters. Try another era or clear the track and vehicle.'}
					</p></Empty
				>{/if}
		{:else if data.left && data.right && !data.era}
			<Empty title="Choose an era">
				<p>Select an era above to compare these drivers.</p>
			</Empty>
		{:else}
			<Empty title="Choose two drivers"
				><p>
					Add drivers from rankings, timing tables or profiles, or enter two LFS
					usernames above.
				</p></Empty
			>
		{/if}
	</div>
</main>
