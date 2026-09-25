<script lang="ts">
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import Choices from '$lib/components/app/Choices.svelte';
	import Empty from '$lib/components/app/Empty.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import Progress from '$lib/components/app/Progress.svelte';
	import { dateTime, delta, lapTime, pageItems } from '$lib/format.js';
	import { queryValue, updateQuery } from '$lib/query.js';
	import { useSession } from '$lib/session.svelte.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const rules = $derived(data.ranking.rules);
	const session = useSession();
	const base = $derived(`/hotlaps/${data.era.id}`);
	const search = $derived(queryValue('q'));
	const status = $derived(
		session.signedIn ? queryValue('status') || 'All' : 'All',
	);

	const rows = $derived(
		data.ranking.charts.map((chart) => ({
			...chart,
			key: `${chart.track}/${chart.vehicle}`,
			lapTimeMs: chart.my_hotlap?.lap_time_ms ?? null,
			position: chart.my_hotlap?.position ?? null,
			distanceToWorldRecordMs:
				chart.my_hotlap?.distance_to_world_record_ms ?? null,
			splits: chart.my_hotlap
				? [
						chart.my_hotlap.split_1_ms,
						chart.my_hotlap.split_2_ms,
						chart.my_hotlap.split_3_ms,
						chart.my_hotlap.split_4_ms,
					]
						.filter((split) => split > 0)
						.slice(0, -1)
				: [],
			createdAt: chart.my_hotlap?.created_at ?? null,
			gameVersion: chart.my_hotlap?.game_version ?? null,
			sprUrl: chart.my_hotlap?.spr_url ?? null,
		})),
	);
	const completed = $derived(
		rows.filter((row) => row.lapTimeMs !== null).length,
	);

	const filtered = $derived(
		rows.filter(
			(row) =>
				row.key
					.toLowerCase()
					.includes(search.toLowerCase().replace(/\s*\/\s*/g, '/')) &&
				(status === 'All' ||
					(status === 'Missing'
						? row.lapTimeMs === null
						: row.lapTimeMs !== null)),
		),
	);
	const paged = $derived(pageItems(filtered, Number(queryValue('page') || 1)));
</script>

<div class="space-y-6">
	<div class="text-sm text-muted-foreground">
		<p class="font-medium text-foreground">How scoring works</p>
		<p class="mt-2">
			Each chart has a benchmark of <strong class="text-foreground"
				>{rules.benchmark_percent}%</strong
			>
			of its world record; a driver's handicap is the total time their fastest lap
			on each chart falls above that benchmark, so the lowest handicap leads. National
			standings award points per chart from a nation's fastest
			<strong class="text-foreground">{rules.nation_driver_limit}</strong>
			drivers, up to
			<strong class="text-foreground">{rules.nation_max_points}</strong> points per
			chart.
		</p>
	</div>
	<div class="flex flex-wrap items-center gap-4">
		<Input
			class="sm:max-w-xs"
			aria-label="Search combinations"
			placeholder="Search combinations..."
			value={search}
			oninput={(e) => updateQuery('q', e.currentTarget.value)}
		/>
		{#if session.signedIn}
			<Choices
				label="Completion"
				value={status}
				options={['All', 'Missing', 'Completed']}
				onValueChange={(v) => updateQuery('status', v)}
			/>
			<div class="sm:ml-auto">
				<Progress value={completed} total={rows.length} />
			</div>
		{/if}
	</div>
	{#if filtered.length}
		<Card.Root class="gap-0 py-0">
			<Card.Content
				class="px-0 [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
			>
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head>Combo</Table.Head>
							<Table.Head class="text-center">#</Table.Head>
							<Table.Head class="w-24 whitespace-nowrap">Lap time</Table.Head>
							<Table.Head
								class="w-24 whitespace-nowrap"
								title="Difference from the current world record"
								>To WR</Table.Head
							>
							<Table.Head class="w-24 whitespace-nowrap" title="Split 1"
								>S1</Table.Head
							>
							<Table.Head class="w-24 whitespace-nowrap" title="Split 2"
								>S2</Table.Head
							>
							<Table.Head class="w-24 whitespace-nowrap" title="Split 3"
								>S3</Table.Head
							>
							<Table.Head>Set</Table.Head>
							<Table.Head>Replay</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each paged.items as row (row.key)}
							<Table.Row>
								<Table.Cell>
									<a
										class="font-mono font-medium hover:underline"
										href="{base}/charts/{row.track}/{row.vehicle}"
									>
										{row.track} / {row.vehicle}
									</a>
								</Table.Cell>
								<Table.Cell
									class="text-center tabular-nums {row.position === 1
										? 'text-time-best'
										: ''}">{row.position ?? '-'}</Table.Cell
								>
								<Table.Cell
									class="w-24 whitespace-nowrap font-mono font-semibold tabular-nums {row.position ===
									1
										? 'text-time-best'
										: ''}">{lapTime(row.lapTimeMs)}</Table.Cell
								>
								<Table.Cell
									class="w-24 whitespace-nowrap font-mono tabular-nums text-muted-foreground"
									>{row.position === 1
										? '-'
										: delta(row.distanceToWorldRecordMs)}</Table.Cell
								>
								{#each [0, 1, 2] as index}
									<Table.Cell
										class="w-24 whitespace-nowrap font-mono tabular-nums text-muted-foreground"
									>
										{row.splits[index] === undefined
											? '-'
											: lapTime(row.splits[index])}
									</Table.Cell>
								{/each}
								<Table.Cell class="whitespace-nowrap text-muted-foreground">
									{#if row.createdAt}
										{dateTime(row.createdAt)} · {row.gameVersion}
									{:else}-{/if}
								</Table.Cell>
								<Table.Cell>
									{#if row.sprUrl}
										<Button
											variant="ghost"
											size="icon"
											href={row.sprUrl}
											aria-label="Download replay for {row.track} / {row.vehicle}"
										>
											<DownloadIcon />
										</Button>
									{:else}
										<span class="text-muted-foreground">-</span>
									{/if}
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</Card.Content>
		</Card.Root>
		<PaginationControls pagination={paged.pagination} label="combinations" />
	{:else if rows.length}
		<Empty title="No matching combinations">
			<p>Try adjusting your search or completion filter.</p>
		</Empty>
	{:else}
		<Empty title="This ranking has no charts yet">
			<p>An operator has not selected the combinations it counts.</p>
		</Empty>
	{/if}
</div>
