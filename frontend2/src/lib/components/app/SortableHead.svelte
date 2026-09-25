<script lang="ts" generics="Column extends string">
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUpDown from '@lucide/svelte/icons/arrow-up-down';
	import * as Table from '$lib/components/ui/table/index.js';
	import type { Ordering } from '$lib/api.js';
	import { setQuery } from '$lib/query.js';

	let {
		column,
		activeColumn,
		order,
		label,
		shortLabel,
		centered = false,
	}: {
		column: Column;
		activeColumn: Column;
		order: Ordering;
		label: string;
		shortLabel?: string;
		centered?: boolean;
	} = $props();
	const active = $derived(column === activeColumn);
	const nextOrder = $derived(active && order === 'asc' ? 'desc' : 'asc');
</script>

<Table.Head
	class={centered ? 'text-center' : undefined}
	aria-sort={active ? (order === 'asc' ? 'ascending' : 'descending') : 'none'}
>
	<button
		type="button"
		class="inline-flex cursor-pointer items-center gap-1 rounded-sm hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring {centered
			? 'w-full justify-center'
			: ''}"
		aria-label={`Sort by ${label}, ${nextOrder === 'asc' ? 'ascending' : 'descending'}`}
		onclick={() => setQuery({ column, order: nextOrder })}
	>
		{shortLabel ?? label}
		{#if !active}
			<ArrowUpDown class="size-4" aria-hidden="true" />
		{:else if order === 'asc'}
			<ArrowUp class="size-4" aria-hidden="true" />
		{:else}
			<ArrowDown class="size-4" aria-hidden="true" />
		{/if}
	</button>
</Table.Head>
