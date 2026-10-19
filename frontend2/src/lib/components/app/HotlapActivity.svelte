<script lang="ts">
	import { page } from '$app/state';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import {
		type Hotlap,
		type EraSummary,
		type HotlapListColumn,
		type Ordering,
		type PaginatedResponseHotlap,
		useApi,
	} from '$lib/api.js';
	import { dateTime, lapTime } from '$lib/format.js';

	import type { ListHotlapsRequest } from '@lfsplanet/sdk/api';
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import Empty from '$lib/components/app/Empty.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import PlayerBadge from '$lib/components/app/PlayerBadge.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import SortableHead from '$lib/components/app/SortableHead.svelte';
	import TableFrame from '$lib/components/app/TableFrame.svelte';
	import MostHotlappedCombos from '$lib/components/app/MostHotlappedCombos.svelte';

	import * as Table from '$lib/components/ui/table/index.js';
	import * as Tabs from '$lib/components/ui/tabs/index.js';

	import { useSession } from '$lib/session.svelte.js';
	import { setQuery } from '$lib/query.js';

	const api = useApi();

	type View =
		'all' | 'world_records' | 'longest_standing' | 'most_hotlapped' | 'mine';

	let {
		eraId,
		lfsUsername,
		refreshKey,
		eras = [],
		views = ['all', 'world_records', 'longest_standing', 'mine'],
		perPage = 25,
		appearance = 'default',
		title = 'Hotlaps',
	}: {
		eraId?: string;
		lfsUsername?: string;
		refreshKey?: unknown;
		eras?: Pick<EraSummary, 'id' | 'title'>[];
		views?: View[];
		perPage?: number;
		appearance?: 'default' | 'data';
		title?: string;
	} = $props();

	const session = useSession();
	let response = $state<PaginatedResponseHotlap | null>(null);
	let loading = $state(true);
	let unavailable = $state(false);
	let requestId = 0;

	const requestedView = $derived(
		((page.url.searchParams.get('view') ?? '') ||
			((page.url.searchParams.get('mine') ?? '') === 'true'
				? 'mine'
				: 'all')) as View,
	);
	const view = $derived(
		views.includes(requestedView) &&
			(requestedView !== 'most_hotlapped' || eraId) &&
			(requestedView !== 'mine' || session.signedIn)
			? requestedView
			: 'all',
	);
	const column = $derived(
		((page.url.searchParams.get('column') ?? '') ||
			'submitted') as HotlapListColumn,
	);
	const order = $derived(
		((page.url.searchParams.get('order') ?? '') ||
			(view === 'longest_standing' ? 'asc' : 'desc')) as Ordering,
	);
	const worldRecords = $derived(
		view === 'world_records' || view === 'longest_standing',
	);
	const filters: ListHotlapsRequest = $derived({
		era_id: eraId,
		lfs_username: lfsUsername,
		state: view === 'mine' ? undefined : 'valid',
		mine: view === 'mine' ? true : undefined,
		rank: worldRecords ? 1 : undefined,
		page: Number((page.url.searchParams.get('page') ?? '') || 1),
		per_page: perPage,
		column,
		order,
	});

	$effect(() => {
		if (
			requestedView !== view ||
			(page.url.searchParams.get('mine') ?? '') === 'true'
		) {
			void setQuery({ view, mine: '', page: '' }, true);
		}
	});

	$effect(() => {
		const refresh = refreshKey;
		if (view === 'most_hotlapped') {
			++requestId;
			return;
		}
		const params = filters;
		void load(params, refresh);
	});

	async function load(params: ListHotlapsRequest, _refresh: unknown) {
		const id = ++requestId;
		loading = true;
		unavailable = false;
		response = null;
		try {
			const result = await api.hotlaps.listHotlaps(params);
			if (id === requestId) response = result;
		} catch {
			if (id === requestId) {
				response = null;
				unavailable = true;
			}
		} finally {
			if (id === requestId) loading = false;
		}
	}

	function changeView(value: string) {
		if (views.includes(value as View)) {
			void setQuery({
				view: value,
				mine: '',
				page: '',
				...(value === 'longest_standing' ||
				view === 'longest_standing' ||
				value === 'most_hotlapped' ||
				view === 'most_hotlapped'
					? { column: '', order: '' }
					: {}),
			});
		}
	}

	const items = $derived(response?.items ?? []);
	const pagination = $derived(response?.pagination);
</script>

<Tabs.Root value={view} onValueChange={changeView} class="min-w-0">
	<div class="flex flex-wrap items-center gap-4">
		<h2
			class={appearance === 'data'
				? 'racing-section-title'
				: 'text-lg font-semibold'}
		>
			{title}
		</h2>
		<Tabs.List
			aria-label="Filter hotlaps"
			class="max-w-full flex-wrap group-data-horizontal/tabs:h-auto"
		>
			{#if views.includes('all')}<Tabs.Trigger value="all"
					>All hotlaps</Tabs.Trigger
				>{/if}
			{#if views.includes('world_records')}
				<Tabs.Trigger value="world_records">World records</Tabs.Trigger>
			{/if}
			{#if views.includes('longest_standing')}
				<Tabs.Trigger value="longest_standing"
					>Longest standing WRs</Tabs.Trigger
				>
			{/if}
			{#if views.includes('most_hotlapped') && eraId}
				<Tabs.Trigger value="most_hotlapped">Most hotlapped combos</Tabs.Trigger
				>
			{/if}
			{#if views.includes('mine') && session.signedIn}
				<Tabs.Trigger value="mine">Mine</Tabs.Trigger>
			{/if}
		</Tabs.List>
	</div>
	<Tabs.Content value={view}>
		{#if view === 'most_hotlapped' && eraId}
			<MostHotlappedCombos {eraId} {perPage} {appearance} {refreshKey} />
		{:else if loading}
			<p class="py-10 text-center text-sm text-muted-foreground" role="status">
				Loading hotlaps...
			</p>
		{:else if unavailable}
			<Empty title="Hotlaps unavailable">
				<p>The hotlap feed could not be loaded.</p>
			</Empty>
		{:else if items.length === 0}
			<Empty
				title={pagination && pagination.total_items > 0
					? 'No hotlaps on this page'
					: view === 'mine'
						? 'No uploads yet'
						: worldRecords
							? 'No world records yet'
							: 'No hotlaps yet'}
			>
				{#if pagination && pagination.total_items > 0}
					<p>Choose another page below.</p>
				{/if}
			</Empty>
			{#if pagination && pagination.total_items > 0}
				<div class="p-4">
					<PaginationControls {pagination} label="hotlaps" />
				</div>
			{/if}
		{:else}
			<TableFrame {appearance}>
				<Table.Root
					class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4"
				>
					<Table.Header>
						<Table.Row>
							<SortableHead
								column="submitted"
								label="Submitted"
								activeColumn={column}
								{order}
							/>
							<SortableHead
								column="driver"
								label="Driver"
								activeColumn={column}
								{order}
							/>
							<Table.Head>Era</Table.Head>
							<Table.Head>Track / Vehicle</Table.Head>
							<SortableHead
								column="rank"
								label="Rank"
								shortLabel="#"
								activeColumn={column}
								{order}
							/>
							<Table.Head>Contributes to</Table.Head>
							<SortableHead
								column="lap_time"
								label="Lap time"
								activeColumn={column}
								{order}
							/>
							{#if view === 'mine'}<Table.Head>State</Table.Head>{/if}
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each items as lap (lap.id)}
							{@const isRecord = lap.position === 1}
							<Table.Row>
								<Table.Cell class="text-muted-foreground">
									<time datetime={lap.created_at}
										>{dateTime(lap.created_at)}</time
									>
								</Table.Cell>
								<Table.Cell>
									<div class="flex min-w-0 items-center gap-1">
										<Flag
											code={lap.player.flag_code}
											fallback={lap.player.country_code}
										/>
										<a
											class="min-w-0 truncate font-medium hover:underline"
											href="/drivers/{encodeURIComponent(
												lap.player.lfs_username,
											)}">{lap.player.display_name}</a
										>
										{#each lap.player.badges ?? [] as badge (badge.id)}
											<PlayerBadge {badge} />
										{/each}
										<CompareButton driver={lap.player} />
									</div>
								</Table.Cell>
								<Table.Cell>
									<a class="hover:underline" href={`/hotlaps/${lap.era_id}`}>
										<Badge variant="secondary"
											>{eras.find((era) => era.id === lap.era_id)?.title ??
												lap.era_id}</Badge
										>
									</a>
								</Table.Cell>
								<Table.Cell>
									{#if lap.vehicle}
										<a
											class="hover:underline"
											href={`/hotlaps/${lap.era_id}/charts/${encodeURIComponent(lap.track)}/${encodeURIComponent(lap.vehicle)}`}
											>{lap.track} / {lap.vehicle}</a
										>
									{:else}{lap.track} / -{/if}
								</Table.Cell>
								<Table.Cell
									class="tabular-nums {isRecord ? 'text-time-best' : ''}"
									>{lap.position ?? '-'}</Table.Cell
								>
								<Table.Cell>
									<div class="flex flex-wrap gap-1">
										{#each lap.contributes_to as ranking (ranking.id)}
											<Badge
												variant="outline"
												href={`/hotlaps/${lap.era_id}/rankings/${encodeURIComponent(ranking.id)}`}
												>{ranking.title}</Badge
											>
										{:else}<span class="text-muted-foreground">-</span>{/each}
									</div>
								</Table.Cell>
								<Table.Cell
									class="font-mono tabular-nums {isRecord
										? 'font-semibold text-time-best'
										: ''}">{lapTime(lap.lap_time_ms)}</Table.Cell
								>
								{#if view === 'mine'}<Table.Cell class="capitalize"
										>{lap.state.replace('_', ' ')}</Table.Cell
									>{/if}
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</TableFrame>
			{#if pagination}
				<div class="py-4">
					<PaginationControls {pagination} label="hotlaps" />
				</div>
			{/if}
		{/if}
	</Tabs.Content>
</Tabs.Root>
