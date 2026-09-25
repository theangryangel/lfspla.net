<script lang="ts">
	import * as Pagination from '$lib/components/ui/pagination/index.js';
	import { updateQuery } from '$lib/query.js';
	import type { Pagination as PaginationMetadata } from '$lib/api.js';

	let {
		pagination,
		label = 'items',
		pageKey = 'page',
	}: {
		pagination: PaginationMetadata;
		label?: string;
		pageKey?: string;
	} = $props();

	function changePage(page: number) {
		const target = Math.max(1, Math.min(page, pagination.total_pages));
		void updateQuery(pageKey, String(target));
	}
</script>

<div
	class="flex flex-wrap items-center justify-between gap-4 text-sm text-muted-foreground"
>
	<span>{pagination.total_items.toLocaleString()} {label}</span>
	{#if pagination.total_pages > 0 || pagination.page > 1}
		<Pagination.Root
			count={pagination.total_items}
			perPage={pagination.per_page}
			page={pagination.page}
			onPageChange={changePage}
			class="mx-0 w-auto"
		>
			{#snippet children({ pages, currentPage })}
				<Pagination.Content>
					<Pagination.Item>
						<Pagination.Previous />
					</Pagination.Item>
					{#each pages as page (page.key)}
						<Pagination.Item>
							{#if page.type === 'ellipsis'}
								<Pagination.Ellipsis />
							{:else}
								<Pagination.Link {page} isActive={currentPage === page.value} />
							{/if}
						</Pagination.Item>
					{/each}
					<Pagination.Item>
						<Pagination.Next />
					</Pagination.Item>
				</Pagination.Content>
			{/snippet}
		</Pagination.Root>
	{/if}
</div>
