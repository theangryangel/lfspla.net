<script lang="ts">
	import PlayerBadge from '#lib/components/app/PlayerBadge.svelte';
	import CompareButton from '#lib/components/app/CompareButton.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import DataSplit from '#lib/components/app/DataSplit.svelte';
	import Panel from '#lib/components/app/Panel.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import NationBreakdown from '#lib/components/app/NationBreakdown.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import Empty from '#lib/components/app/Empty.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
	import { delta, relativeColor } from '#lib/format.js';
	import { useSession } from '#lib/session.svelte.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const session = useSession();
	const driverRows = $derived(data.players?.entries ?? []);
	const nationRows = $derived(data.nations?.entries ?? []);
</script>

{#snippet standingsTable(nations: boolean)}
	{@const standings = nations ? 'Nations' : 'Drivers'}
	{@const rows = nations ? nationRows : driverRows}
	{@const available = nations ? data.nations : data.players}
	<Panel
		titleId={nations ? 'nation-standings-title' : 'driver-standings-title'}
		title={standings}
		flush
		surface={false}
		appearance="data"
	>
		{#if !available}
			<Empty title="Standings are not available">
				<p>
					{data.ranking.title} has no charts selected yet, so there is nothing to
					rank.
				</p>
			</Empty>
		{:else if !rows.length}
			<Empty title="No standings yet">
				<p>No validated laps have been submitted for this ranking.</p>
			</Empty>
		{:else}
			<TableFrame appearance="data">
				<Table.Root aria-label={`${standings} standings`}>
					<Table.Header>
						<Table.Row>
							<Table.Head class="text-center">#</Table.Head>
							<Table.Head>{nations ? 'Nation' : 'Driver'}</Table.Head>
							{#if nations}
								<Table.Head>Points</Table.Head>
								<Table.Head>Laps</Table.Head>
							{/if}
							<Table.Head>Charts</Table.Head>
							<Table.Head class="text-right">Handicap</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if nations}
							{#each nationRows as row (row.country_code)}
								<Table.Row>
									<Table.Cell
										class={row.position === 1
											? 'text-center text-time-best shadow-[inset_3px_0_0_var(--time-best)]'
											: 'text-center'}>{row.position}</Table.Cell
									>
									<Table.Cell>
										<div class="flex items-center gap-2 font-medium">
											<Flag code={row.country_code} />
											{row.country_code}
											<NationBreakdown
												era={data.era.id}
												ranking={data.ranking.id}
												country={row.country_code}
											/>
										</div>
									</Table.Cell>
									<Table.Cell class="tabular-nums"
										>{row.points.toLocaleString()}</Table.Cell
									>
									<Table.Cell class="tabular-nums">
										{row.contributing_laps.toLocaleString()}
									</Table.Cell>
									<Table.Cell class="tabular-nums">
										{row.contributing_charts.toLocaleString()} / {data.nations?.total_charts.toLocaleString()}
									</Table.Cell>
									<Table.Cell
										class="text-right font-mono tabular-nums {relativeColor(
											row.handicap_ms,
											0,
										)}">{delta(row.handicap_ms)}</Table.Cell
									>
								</Table.Row>
							{/each}
						{:else}
							{#each driverRows as row (row.player.id)}
								<Table.Row
									data-state={row.player.id === session.player?.id
										? 'selected'
										: undefined}
								>
									<Table.Cell
										class={row.position === 1
											? 'text-center text-time-best shadow-[inset_3px_0_0_var(--time-best)]'
											: 'text-center'}>{row.position}</Table.Cell
									>
									<Table.Cell>
										<div class="flex w-max items-center gap-1">
											<Flag
												code={row.player.flag_code}
												fallback={row.player.country_code}
											/>
											<a
												class="shrink-0 whitespace-nowrap font-medium hover:underline"
												href="/drivers/{row.player.lfs_username}"
											>
												{row.player.display_name}
											</a>
											{#if row.player.id === session.player?.id}
												<Badge variant="secondary">You</Badge>
											{/if}
											{#each row.badges as playerBadge, i (i)}
												<PlayerBadge badge={playerBadge} />
											{/each}
											<CompareButton driver={row.player} />
										</div>
									</Table.Cell>
									<Table.Cell class="tabular-nums">
										{row.completed_charts.toLocaleString()} / {row.total_charts.toLocaleString()}
									</Table.Cell>
									<Table.Cell
										class="text-right font-mono tabular-nums {relativeColor(
											row.handicap_ms,
											0,
										)}">{delta(row.handicap_ms)}</Table.Cell
									>
								</Table.Row>
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</TableFrame>
		{/if}
	</Panel>
{/snippet}

<DataSplit balanced secondaryLabelledBy="nation-standings-title">
	{@render standingsTable(false)}
	{#snippet secondary()}
		{@render standingsTable(true)}
	{/snippet}
</DataSplit>
