<script lang="ts">
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import Empty from '$lib/components/app/Empty.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import SortableHead from '$lib/components/app/SortableHead.svelte';
	import TableFrame from '$lib/components/app/TableFrame.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Tabs from '$lib/components/ui/tabs/index.js';
	import {
		get,
		query,
		type EraSummary,
		type Hotlap,
		type HotlapListColumn,
		type Ordering,
		type PaginatedResponse,
	} from '$lib/api.js';
	import { useSession } from '$lib/session.svelte.js';
	import { setQuery, queryValue } from '$lib/query.js';
	import { dateTime, lapTime } from '$lib/format.js';
	import { hotlapPath } from '$lib/era.js';

	type View = 'all' | 'world_records' | 'mine';

	let {
		eraId,
		lfsUsername,
		refreshKey,
		eras = [],
		views = ['all', 'world_records', 'mine'],
		perPage = 25,
	}: {
		eraId?: string;
		lfsUsername?: string;
		refreshKey?: unknown;
		eras?: Pick<EraSummary, 'id' | 'title'>[];
		views?: View[];
		perPage?: number;
	} = $props();

	const session = useSession();
	let response = $state<PaginatedResponse<Hotlap> | null>(null);
	let loading = $state(true);
	let unavailable = $state(false);
	let requestId = 0;

	const requestedView = $derived(
		(queryValue('view') ||
			(queryValue('mine') === 'true' ? 'mine' : 'all')) as View,
	);
	const view = $derived(
		views.includes(requestedView) &&
			(requestedView !== 'mine' || session.signedIn)
			? requestedView
			: 'all',
	);
	const column = $derived(
		(queryValue('column') || 'submitted') as HotlapListColumn,
	);
	const order = $derived((queryValue('order') || 'desc') as Ordering);
	const endpoint = $derived(
		'/api/v1/hotlaps' +
			query({
				era_id: eraId,
				lfs_username: lfsUsername,
				state: view === 'mine' ? undefined : 'valid',
				mine: view === 'mine' ? 'true' : undefined,
				rank: view === 'world_records' ? 1 : undefined,
				page: queryValue('page') || 1,
				per_page: perPage,
				column,
				order,
			}),
	);

	$effect(() => {
		if (requestedView !== view || queryValue('mine') === 'true') {
			void setQuery({ view, mine: '', page: '' }, true);
		}
	});

	$effect(() => {
		const refresh = refreshKey;
		const url = endpoint;
		void load(url, refresh);
	});

	async function load(url: string, _refresh: unknown) {
		const id = ++requestId;
		loading = true;
		unavailable = false;
		response = null;
		try {
			const result = await get<PaginatedResponse<Hotlap>>(
				fetch,
				url,
			);
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
		if (value === 'all' || value === 'world_records' || value === 'mine') {
			void setQuery({ view: value, mine: '', page: '' });
		}
	}

	const items = $derived(response?.items ?? []);
	const pagination = $derived(response?.pagination);
</script>

<Tabs.Root value={view} onValueChange={changeView} class="min-w-0">
	<div class="flex flex-wrap items-center gap-4">
		<h2 class="text-lg font-semibold">Hotlaps</h2>
		<Tabs.List aria-label="Filter hotlaps">
			{#if views.includes('all')}<Tabs.Trigger value="all"
					>All hotlaps</Tabs.Trigger
				>{/if}
			{#if views.includes('world_records')}
				<Tabs.Trigger value="world_records">World records</Tabs.Trigger>
			{/if}
			{#if views.includes('mine') && session.signedIn}
				<Tabs.Trigger value="mine">Mine</Tabs.Trigger>
			{/if}
		</Tabs.List>
	</div>
	<Tabs.Content value={view}>
		{#if loading}
			<p class="py-10 text-center text-sm text-muted-foreground" role="status">
				Loading hotlaps…
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
						: view === 'world_records'
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
			<TableFrame>
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
										<CompareButton driver={lap.player} />
									</div>
								</Table.Cell>
								<Table.Cell>
									<a class="hover:underline" href={hotlapPath(lap.era_id)}>
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
											href={`${hotlapPath(lap.era_id)}/charts/${encodeURIComponent(lap.track)}/${encodeURIComponent(lap.vehicle)}`}
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
												href={`${hotlapPath(lap.era_id)}/rankings/${encodeURIComponent(ranking.id)}`}
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
