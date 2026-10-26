<script lang="ts">
	import FilterBar from './FilterBar.svelte';
	import { selects, ranges, sorts } from '#lib/vehicle-explorer.js';
	import { useApi } from '#lib/api.js';
	import { setQuery } from '#lib/query.js';
	import type { FilterDefinition } from '#lib/filter-bar.js';

	let {
		filters,
		loading,
	}: { filters: Record<string, string>; loading: boolean } = $props();
	const api = useApi();
	const definitions: FilterDefinition[] = [
		{ key: 'q', label: 'Name or code', placeholder: 'Name or code...' },
		...selects.map((select) => ({
			...select,
			options: select.options.map(([value, label]) => ({ value, label })),
		})),
		...ranges.map((range) => ({ key: range.key, label: range.label, range })),
		{
			key: 'author',
			label: 'Uploaded by',
			placeholder: 'Username or display name...',
			loadOptions: async (query, signal) => {
				const response = await api.players.search(
					{ q: query || undefined, vehicle_author: true, per_page: 10 },
					{ abortSignal: signal },
				);
				return response.items.map((player) => ({
					value: player.lfs_username,
					label: player.display_name,
					description: player.lfs_username,
				}));
			},
		},
	];
</script>

<FilterBar
	{filters}
	{loading}
	{definitions}
	label="Vehicle filters"
	sort={{
		key: 'sort',
		label: 'Sort vehicles',
		defaultValue: 'catalogue',
		options: sorts.map(([value, label]) => ({ value, label })),
	}}
	onApply={(values) => setQuery({ ...values, page: '' })}
/>
