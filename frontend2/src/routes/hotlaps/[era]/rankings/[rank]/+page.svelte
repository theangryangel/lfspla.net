<script lang="ts">
	import CircleHelpIcon from '@lucide/svelte/icons/circle-help';
	import { Button } from '$lib/components/ui/button/index.js';
	import PlayerBadge from '$lib/components/app/PlayerBadge.svelte';
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import Panel from '$lib/components/app/Panel.svelte';
	import TableFrame from '$lib/components/app/TableFrame.svelte';
	import NationBreakdown from '$lib/components/app/NationBreakdown.svelte';
	import * as Table from '$lib/components/ui/table/index.js';
	import Empty from '$lib/components/app/Empty.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import { delta, pageItems, relativeColor } from '$lib/format.js';
	import { queryValue } from '$lib/query.js';
	import { useSession } from '$lib/session.svelte.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const session = useSession();
	const driverRows = $derived(
		pageItems(data.players?.entries ?? [], Number(queryValue('page') || 1)),
	);
	const nationRows = $derived(
		pageItems(
			data.nations?.entries ?? [],
			Number(queryValue('nation_page') || 1),
		),
	);
</script>

<div class="space-y-6">
	<div class="flex justify-end">
		<Button
			variant="outline"
			href={`/hotlaps/${data.era.id}/rankings/${data.ranking.id}/combinations`}
		>
			<CircleHelpIcon />
			Rank Info
		</Button>
	</div>
	<div class="grid items-start gap-6 xl:grid-cols-2">
		{#each ['Drivers', 'Nations'] as standings}
			{@const nations = standings === 'Nations'}
			{@const paged = nations ? nationRows : driverRows}
			{@const available = nations ? data.nations : data.players}
			<div class="min-w-0">
				<Panel title={standings} flush surface={false}>
					{#if !available}
						<Empty title="Standings are not available">
							<p>
								{data.ranking.title} has no charts selected yet, so there is nothing
								to rank.
							</p>
						</Empty>
					{:else if !paged.pagination.total_items}
						<Empty title="No standings yet">
							<p>No validated laps have been submitted for this ranking.</p>
						</Empty>
					{:else}
						<TableFrame>
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
										{#each nationRows.items as row (row.country_code)}
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
										{#each driverRows.items as row (row.player.id)}
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
													<div class="flex min-w-0 items-center gap-1">
														<Flag
															code={row.player.flag_code}
															fallback={row.player.country_code}
														/>
														<a
															class="min-w-0 truncate hover:underline"
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
						<div class="px-(--card-spacing) py-4">
							<PaginationControls
								pagination={paged.pagination}
								label={nations ? 'nations' : 'drivers'}
								pageKey={nations ? 'nation_page' : 'page'}
							/>
						</div>
					{/if}
				</Panel>
			</div>
		{/each}
	</div>
</div>
