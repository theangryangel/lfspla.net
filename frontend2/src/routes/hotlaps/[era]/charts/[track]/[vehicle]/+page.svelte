<script lang="ts">
	import FloatingPanel from '$lib/components/app/FloatingPanel.svelte';
	import HotlapComparison from '$lib/components/app/HotlapComparison.svelte';
	import GitCompare from '@lucide/svelte/icons/git-compare';
	import { mergeProps } from 'bits-ui';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { intermediateSplits } from '$lib/hotlap-comparison.js';
	import SortableHead from '$lib/components/app/SortableHead.svelte';
	import PlayerBadge from '$lib/components/app/PlayerBadge.svelte';
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import ChoiceSelect from '$lib/components/app/ChoiceSelect.svelte';
	import CombinationCard from '$lib/components/app/CombinationCard.svelte';
	import { useCombinationPicker } from '$lib/picker.js';
	import SearchSelect from '$lib/components/app/SearchSelect.svelte';
	import Empty from '$lib/components/app/Empty.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import {
		controlTags,
		dateTime,
		delta,
		lapTime,
		steeringLabels,
	} from '$lib/format.js';
	import { page } from '$app/state';
	import { queryValue, setQuery, updateQuery } from '$lib/query.js';
	import { useSession } from '$lib/session.svelte.js';
	import {
		type BestHotlapColumn,
		type BestHotlapResponse,
		type Ordering,
		type CountrySummary,
	} from '$lib/api.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const session = useSession();
	const entries = $derived(data.chart?.items ?? []);
	const picker = useCombinationPicker();
	const chosen = $derived(Boolean(data.track && data.vehicle));

	const unavailable = $derived.by(() => {
		const track = page.params.track;
		const vehicle = page.params.vehicle;
		switch (data.reason) {
			case 'unknown_track':
				return `There is no Live for Speed track configuration called ${track}.`;
			case 'open_configuration':
				return `${track} is an open configuration, so it has no lap to time.`;
			case 'unknown_vehicle':
				return `There is no Live for Speed vehicle called ${vehicle}.`;
			default:
				return `${data.era.title} does not pair ${vehicle} with ${track}.`;
		}
	});

	const filtered = $derived(
		Boolean(queryValue('country') || queryValue('controller')),
	);
	const column = $derived((queryValue('column') || 'rank') as BestHotlapColumn);
	const order = $derived((queryValue('order') || 'asc') as Ordering);
	const record = $derived(
		entries.length
			? entries[0].lap_time_ms - entries[0].distance_to_world_record_ms
			: null,
	);

	const combination = $derived(
		JSON.stringify([data.era.id, page.params.track, page.params.vehicle]),
	);
	let selection = $state<{ combination: string; laps: BestHotlapResponse[] }>({
		combination: '',
		laps: [],
	});
	const selectedLaps = $derived(
		selection.combination === combination ? selection.laps : [],
	);
	$effect(() => {
		if (selection.combination !== combination)
			selection = { combination, laps: [] };
	});
	function toggleLap(lap: BestHotlapResponse) {
		selection = {
			combination,
			laps: selectedLaps.some((entry) => entry.id === lap.id)
				? selectedLaps.filter((entry) => entry.id !== lap.id)
				: selectedLaps.length < 2
					? [...selectedLaps, lap]
					: selectedLaps,
		};
	}

	const ANY_NATION = { value: '', label: 'All nations' };
	const countryOption = (c: CountrySummary) => ({
		value: c.code,
		label: c.name,
		hint: c.code,
	});

	const countryOptions = $derived([
		ANY_NATION,
		...data.countries.map(countryOption),
	]);
	const controllerOptions = [
		{ value: '', label: 'All controllers' },
		...Object.entries(steeringLabels).map(([value, label]) => ({
			value,
			label,
		})),
	];

	const searchCountries = async (term: string) => {
		const search = term.trim().toLowerCase();
		return [
			ANY_NATION,
			...data.countries
				.filter(
					(country) =>
						country.code.toLowerCase().includes(search) ||
						country.name.toLowerCase().includes(search),
				)
				.map(countryOption),
		];
	};
</script>

<div class="flex flex-wrap items-center gap-4">
	<CombinationCard track={data.track} vehicle={data.vehicle} />
	<div class="flex flex-wrap items-center gap-4 sm:ml-auto">
		{#if record !== null}
			<p class="text-sm text-muted-foreground">
				Record · <span class="font-mono tabular-nums">{lapTime(record)}</span>
			</p>
		{/if}
		{#if chosen}
			<SearchSelect
				label="Nation"
				value={queryValue('country')}
				selectedLabel={data.country?.name ?? ''}
				options={countryOptions}
				search={searchCountries}
				placeholder="Search nations..."
				onValueChange={(v) => updateQuery('country', v)}
			/>
			<ChoiceSelect
				label="Controller"
				value={queryValue('controller')}
				options={controllerOptions}
				onValueChange={(v) => updateQuery('controller', v)}
			/>
			{#if filtered}
				<Button
					variant="ghost"
					onclick={() => setQuery({ country: '', controller: '' })}
				>
					Clear filters
				</Button>
			{/if}
		{/if}
	</div>
</div>
<div class="min-w-0 space-y-4">
	{#if selectedLaps.length}
		<FloatingPanel placement="top-right">
			<HotlapComparison
				laps={selectedLaps}
				onRemove={toggleLap}
				onClear={() => (selection = { combination, laps: [] })}
			/>
		</FloatingPanel>
	{/if}
	<div class="min-w-0 space-y-4">
		{#if !chosen}
			<Empty title="No chart for that combination">
				<p>{unavailable}</p>
				<Button onclick={(event) => picker.open({ from: event.currentTarget })}
					>Choose a combination</Button
				>
			</Empty>
		{:else if entries.length}
			<Card.Root class="gap-0 py-0">
				<Card.Content
					class="px-0 [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
				>
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<SortableHead
									column="rank"
									activeColumn={column}
									{order}
									label="Rank"
									shortLabel="#"
									centered
								/>
								<SortableHead
									column="driver"
									activeColumn={column}
									{order}
									label="Driver"
								/>
								<Table.Head class="w-28 min-w-28 max-w-28 whitespace-nowrap"
									>Lap time</Table.Head
								>
								<Table.Head
									class="w-24 min-w-24 max-w-24 whitespace-nowrap"
									title="Difference from the current world record"
									>To WR</Table.Head
								>
								<Table.Head
									class="w-24 min-w-24 max-w-24 whitespace-nowrap"
									title="Split 1">S1</Table.Head
								>
								<Table.Head
									class="w-24 min-w-24 max-w-24 whitespace-nowrap"
									title="Split 2">S2</Table.Head
								>
								<Table.Head
									class="w-24 min-w-24 max-w-24 whitespace-nowrap"
									title="Split 3">S3</Table.Head
								>
								<Table.Head>Controls</Table.Head>
								<SortableHead
									column="set"
									activeColumn={column}
									{order}
									label="Set"
								/>
								<Table.Head>Replay</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each entries as entry (entry.id)}
								{@const isRecord = entry.position === 1}
								{@const lapSelected = selectedLaps.some(
									(lap) => lap.id === entry.id,
								)}
								{@const splits = intermediateSplits(entry)}
								<Table.Row
									data-state={lapSelected ||
									entry.player.id === session.player?.id
										? 'selected'
										: undefined}
								>
									<Table.Cell
										class="text-center {isRecord
											? 'text-time-best shadow-[inset_3px_0_0_var(--time-best)]'
											: ''}"
									>
										{entry.position}
									</Table.Cell>
									<Table.Cell>
										<div class="flex min-w-0 items-center gap-1">
											<Flag code={entry.player.country_code} />
											<a
												class="min-w-0 truncate hover:underline"
												href="/drivers/{entry.player.lfs_username}"
											>
												{entry.player.display_name}
											</a>
											{#each entry.player.badges as playerBadge, i (i)}
												<PlayerBadge badge={playerBadge} />
											{/each}
											<CompareButton driver={entry.player} />
										</div>
									</Table.Cell>
									<Table.Cell
										class="w-28 min-w-28 max-w-28 whitespace-nowrap font-mono font-semibold tabular-nums {isRecord
											? 'text-time-best'
											: ''}"
									>
										<div class="flex items-center justify-between gap-2">
											<span>{lapTime(entry.lap_time_ms)}</span>
											<Tooltip.Root>
												<Tooltip.Trigger>
													{#snippet child({ props })}
														<Button
															{...mergeProps(props, {
																onclick: () => toggleLap(entry),
															})}
															variant={lapSelected ? 'outline' : 'ghost'}
															size="icon-sm"
															class="size-7 shrink-0"
															aria-pressed={lapSelected}
															disabled={!lapSelected &&
																selectedLaps.length === 2}
															aria-label={`${lapSelected ? 'Remove' : 'Compare'} ${entry.player.display_name}'s lap, ${lapTime(entry.lap_time_ms)}`}
														>
															<GitCompare class="size-4" aria-hidden="true" />
														</Button>
													{/snippet}
												</Tooltip.Trigger>
												<Tooltip.Content>
													{lapSelected
														? 'Remove lap from comparison'
														: 'Compare lap'}
												</Tooltip.Content>
											</Tooltip.Root>
										</div>
									</Table.Cell>
									<Table.Cell
										class="w-24 min-w-24 max-w-24 whitespace-nowrap font-mono tabular-nums text-muted-foreground"
									>
										{isRecord ? '-' : delta(entry.distance_to_world_record_ms)}
									</Table.Cell>
									{#each [0, 1, 2] as index}
										<Table.Cell
											class="w-24 min-w-24 max-w-24 whitespace-nowrap font-mono tabular-nums text-muted-foreground"
										>
											{splits[index] === undefined
												? '-'
												: lapTime(splits[index])}
										</Table.Cell>
									{/each}
									<Table.Cell>
										<div class="flex max-w-40 flex-wrap gap-1">
											<Badge variant="secondary"
												>{steeringLabels[entry.steering]}</Badge
											>
											{#each controlTags(entry) as tag (tag)}
												<Badge variant="outline">{tag}</Badge>
											{/each}
										</div>
									</Table.Cell>
									<Table.Cell class="text-muted-foreground">
										{dateTime(entry.created_at)} · {entry.game_version}
									</Table.Cell>
									<Table.Cell>
										{#if entry.spr_url}
											<Button
												variant="ghost"
												size="icon"
												href={entry.spr_url}
												aria-label="Download {entry.player
													.display_name}'s replay"
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
		{:else}
			<Empty
				title={data.chart?.pagination.total_items
					? 'No drivers on this page'
					: 'No laps on this chart'}
			>
				<p>
					{data.chart?.pagination.total_items
						? 'Choose an earlier page to see drivers on this chart.'
						: filtered
							? 'No validated laps match these filters.'
							: 'No validated laps have been submitted for this combination yet.'}
				</p>
			</Empty>
		{/if}

		{#if data.chart}
			<PaginationControls pagination={data.chart.pagination} label="drivers" />
		{/if}
	</div>
</div>
