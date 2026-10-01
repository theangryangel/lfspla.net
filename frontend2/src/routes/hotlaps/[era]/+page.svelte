<script lang="ts">
	import HotlapActivity from '$lib/components/app/HotlapActivity.svelte';
	import TableFrame from '$lib/components/app/TableFrame.svelte';
	import * as Table from '$lib/components/ui/table/index.js';
	import Flag from '$lib/components/app/Flag.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<div
	class="grid min-w-0 items-start gap-6 xl:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]"
>
	<div class="min-w-0">
		<HotlapActivity
			eraId={data.era.id}
			refreshKey={data.holders}
			eras={data.eras}
			views={['all', 'world_records', 'mine']}
			perPage={25}
		/>
	</div>
	<div class="min-w-0">
		<section class="space-y-3">
			<h2 class="text-lg font-semibold">World record holders</h2>
			<TableFrame>
				<Table.Root
					class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4"
				>
					<Table.Header>
						<Table.Row>
							<Table.Head>#</Table.Head>
							<Table.Head>Driver</Table.Head>
							<Table.Head class="text-right">World records</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each data.holders as holder (holder.player.id)}
							<Table.Row>
								<Table.Cell class="tabular-nums">{holder.position}</Table.Cell>
								<Table.Cell>
									<Flag
										code={holder.player.flag_code}
										fallback={holder.player.country_code}
										class="mr-1"
									/>
									<a
										class="font-medium hover:underline"
										href="/drivers/{encodeURIComponent(
											holder.player.lfs_username,
										)}">{holder.player.display_name}</a
									>
								</Table.Cell>
								<Table.Cell class="text-right font-semibold tabular-nums"
									>{holder.world_records.toLocaleString()}</Table.Cell
								>
							</Table.Row>
						{:else}
							<Table.Row>
								<Table.Cell
									colspan={3}
									class="py-8 text-center text-muted-foreground"
								>
									{data.pagination.total_items > 0
										? 'No holders on this page. Choose another page below.'
										: 'No world records in this era yet.'}
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</TableFrame>
			<PaginationControls
				pagination={data.pagination}
				label="holders"
				pageKey="wr_page"
			/>
		</section>
	</div>
</div>
