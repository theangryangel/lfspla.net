<script lang="ts">
	import { page } from '$app/state';
	import {
		useApi,
		type PaginatedResponseCombination,
		type Ordering,
	} from '#lib/api.js';
	import type { ListEraCombinationsRequest } from '@lfsplanet/sdk/api';
	import Empty from '#lib/components/app/Empty.svelte';
	import PaginationControls from '#lib/components/app/PaginationControls.svelte';
	import SortableHead from '#lib/components/app/SortableHead.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import * as Table from '#lib/components/ui/table/index.js';

	let {
		eraId,
		perPage,
		appearance,
		refreshKey,
	}: {
		eraId: string;
		perPage: number;
		appearance: 'default' | 'data';
		refreshKey?: unknown;
	} = $props();

	const api = useApi();
	let response = $state<PaginatedResponseCombination | null>(null);
	let loading = $state(true);
	let unavailable = $state(false);
	let requestId = 0;
	const column = $derived(
		(page.url.searchParams.get('column') || 'participants') as NonNullable<
			ListEraCombinationsRequest['column']
		>,
	);
	const order = $derived(
		(page.url.searchParams.get('order') || 'desc') as Ordering,
	);
	const filters: ListEraCombinationsRequest = $derived({
		era: eraId,
		column,
		order,
		page: Number(page.url.searchParams.get('page') || 1),
		per_page: perPage,
	});

	$effect(() => {
		const refresh = refreshKey;
		void load(filters, refresh);
	});

	async function load(params: ListEraCombinationsRequest, _refresh: unknown) {
		const id = ++requestId;
		loading = true;
		unavailable = false;
		response = null;
		try {
			const result = await api.eras.listEraCombinations(params);
			if (id === requestId) response = result;
		} catch {
			if (id === requestId) unavailable = true;
		} finally {
			if (id === requestId) loading = false;
		}
	}

	const pagination = $derived(response?.pagination);
</script>

<p class="mb-3 text-sm text-muted-foreground">
	Ranked by drivers with a current valid personal best on each combo.
</p>
{#if loading}
	<p class="py-10 text-center text-sm text-muted-foreground" role="status">
		Loading combos...
	</p>
{:else if unavailable}
	<Empty title="Combos unavailable"
		><p>The combinations could not be loaded.</p></Empty
	>
{:else if !response?.items.length}
	<Empty
		title={pagination && pagination.total_items > 0
			? 'No combos on this page'
			: 'No combos yet'}
	/>
{:else}
	<TableFrame {appearance}>
		<Table.Root
			class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4"
		>
			<Table.Header>
				<Table.Row>
					<SortableHead
						column="track"
						label="Track / Vehicle"
						activeColumn={column}
						{order}
					/>
					<SortableHead
						column="participants"
						label="Drivers"
						activeColumn={column}
						{order}
					/>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each response.items as combo (`${combo.track.code}/${combo.vehicle.code}`)}
					{@const href = `/hotlaps/${combo.era_id}/charts/${encodeURIComponent(combo.track.code)}/${encodeURIComponent(combo.vehicle.code)}`}
					<Table.Row>
						<Table.Cell class="py-3!">
							<a
								class="group inline-flex flex-col gap-0.5 whitespace-normal"
								{href}
							>
								<span class="font-medium group-hover:underline"
									>{combo.track.name} / {combo.vehicle.name}</span
								>
								<span class="text-sm text-muted-foreground"
									>{combo.track.code} / {combo.vehicle.code}</span
								>
							</a>
						</Table.Cell>
						<Table.Cell class="py-3! font-semibold tabular-nums"
							>{combo.participants.toLocaleString()}</Table.Cell
						>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	</TableFrame>
{/if}
{#if pagination && pagination.total_items > 0}
	<div class="py-4"><PaginationControls {pagination} label="combos" /></div>
{/if}
