<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import Thumbnail from '$lib/components/app/Thumbnail.svelte';
	import { CARD } from '$lib/components/app/picker.js';
	import {
		getList,
		query,
		type TrackSummary,
		type VehicleSummary,
	} from '$lib/api.js';

	let {
		era,
		track = null,
		selected = null,
		autofocus = false,
		height = 'max-h-[60vh]',
		trailing,
		onSelect,
	}: {
		era: string;
		/** The track whose pairings are offered, so a pick is always valid. A
		 * pick from the era's whole catalogue narrows the tracks instead. */
		track?: TrackSummary | null;
		selected?: VehicleSummary | null;
		autofocus?: boolean;
		height?: string;
		/** Sits after the search box, for whatever the search is narrowed to. */
		trailing?: Snippet;
		onSelect: (vehicle: VehicleSummary) => void;
	} = $props();

	const CAP = 100;
	let term = $state('');
	let filters = $state<HTMLDivElement | null>(null);
	let vehicles = $state<VehicleSummary[]>([]);
	let capped = $state(false);
	let pending = $state(true);
	let failed = $state(false);

	// Ignore stale requests.
	let latest = 0;
	let timer: ReturnType<typeof setTimeout> | undefined;

	const contains = (text: string) =>
		text.toLowerCase().includes(term.trim().toLowerCase());
	const shown = $derived(
		capped
			? vehicles
			: vehicles.filter((v) => contains(v.name) || contains(v.code)),
	);

	async function load(search: string) {
		const token = ++latest;
		pending = true;
		try {
			const found = await getList<VehicleSummary>(
				fetch,
				`/api/v1/eras/${encodeURIComponent(era)}/vehicles` +
					query({ track: track?.code, q: search, limit: CAP }),
			);
			if (token !== latest) return;
			vehicles = found;
			// Search remotely when the full set exceeds the cap.
			if (!search) capped = found.length >= CAP;
			failed = false;
		} catch {
			if (token !== latest) return;
			vehicles = [];
			failed = true;
		} finally {
			if (token === latest) pending = false;
		}
	}

	function onSearch(next: string) {
		if (!capped) return;
		clearTimeout(timer);
		// Cancel the pending search.
		latest += 1;
		pending = true;
		failed = false;
		timer = setTimeout(() => load(next), 200);
	}

	$effect(() => {
		track?.code;
		term = '';
		capped = false;
		load('');
	});

	onMount(() => {
		// Moving between steps keeps focus in the dialog without opening a
		// keyboard over the grid.
		if (autofocus) filters?.focus({ preventScroll: true });
	});

	$effect(() => () => clearTimeout(timer));
</script>

<div
	bind:this={filters}
	role="group"
	aria-label="Vehicle filters"
	tabindex="-1"
	class="flex flex-wrap items-center gap-2 outline-none"
>
	<div class="relative w-full flex-1 sm:w-auto sm:min-w-48">
		<SearchIcon
			class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
		/>
		<Input
			class="h-9 pl-8"
			aria-label="Search vehicles"
			placeholder="Search vehicles..."
			value={term}
			oninput={(event) => {
				term = event.currentTarget.value;
				onSearch(term);
			}}
		/>
	</div>
	{@render trailing?.()}
</div>
<div class="-mx-1 overflow-y-auto px-1 {height}">
	{#if pending}
		<p class="py-12 text-center text-sm text-muted-foreground">
			Loading vehicles...
		</p>
	{:else if shown.length}
		<ul class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
			{#each shown as option (option.code)}
				<li>
					<button
						type="button"
						aria-current={option.code === selected?.code ? 'true' : undefined}
						class={CARD}
						onclick={() => onSelect(option)}
					>
						<Thumbnail
							kind="vehicle"
							code={option.code}
							src={option.image_url}
						/>
						<span class="flex items-center gap-1.5 p-2">
							<span class="min-w-0 flex-1">
								<span class="block truncate text-sm font-medium"
									>{option.name}</span
								>
								<span class="block font-mono text-xs text-muted-foreground"
									>{option.code}</span
								>
							</span>
							<Badge variant="secondary">{option.license}</Badge>
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="py-12 text-center text-sm text-muted-foreground">
			{#if failed}
				The vehicles for this track could not be loaded.
			{:else if term}
				No vehicles match that search.
			{:else if track}
				This era pairs no vehicles with {track.name}.
			{:else}
				This era offers no vehicles yet.
			{/if}
		</p>
	{/if}
</div>
