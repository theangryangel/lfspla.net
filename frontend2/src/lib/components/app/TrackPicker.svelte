<script module lang="ts">
	import type { TrackSummary } from '#lib/api.js';

	// Share catalogue requests across picker instances and dialog reopenings.
	const requests = new Map<string, Promise<TrackSummary[]>>();
</script>

<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';

	import { onMount, type Snippet } from 'svelte';
	import SearchIcon from '@lucide/svelte/icons/search';

	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import TrackCard from '#lib/components/app/TrackCard.svelte';
	import { useApi, type VehicleSummary } from '#lib/api.js';

	const api = useApi();

	let {
		era,
		selected = null,
		carrying = null,
		autofocus = false,
		height = 'max-h-[60vh]',
		trailing,
		onSelect,
	}: {
		era: string;
		/** The track in use, marked in the grid. */
		selected?: TrackSummary | null;
		/** A vehicle to keep, which narrows the grid to the tracks offering it,
		 * so a pick is always valid. */
		carrying?: VehicleSummary | null;
		autofocus?: boolean;
		height?: string;
		/** Sits after the search box, for whatever the search is narrowed to. */
		trailing?: Snippet;
		onSelect: (track: TrackSummary) => void;
	} = $props();

	let term = $state('');
	let locationFilter = $state<string>('all');
	let filters = $state<HTMLDivElement | null>(null);
	let tracks = $state<TrackSummary[] | null>(null);
	let failed = $state(false);
	let retry = $state(0);

	const contains = (text: string) =>
		text.toLowerCase().includes(term.trim().toLowerCase());
	const locations = $derived(
		Array.from(
			new Map(
				(tracks ?? []).map((track) => [track.location.code, track.location]),
			).values(),
		).sort((a, b) => a.sequence - b.sequence || a.code.localeCompare(b.code)),
	);
	const shown = $derived(
		(tracks ?? []).filter(
			(track) =>
				(locationFilter === 'all' || track.location.code === locationFilter) &&
				(contains(track.name) || contains(track.code)),
		),
	);

	$effect(() => {
		retry;
		const vehicle = carrying?.code ?? null;
		let active = true;
		tracks = null;
		failed = false;
		const key = JSON.stringify([era, vehicle || null]);
		let request = requests.get(key);
		if (!request) {
			request = api.tracks
				.listEraTracks({ era, vehicle: vehicle || undefined })
				.then((response) => response.items)
				.catch((cause) => {
					requests.delete(key);
					throw cause;
				});
			requests.set(key, request);
		}
		request
			.then((found) => {
				if (active) tracks = found;
			})
			.catch(() => {
				if (active) failed = true;
			});
		return () => {
			active = false;
		};
	});

	onMount(() => {
		// Moving between steps keeps focus in the dialog without opening a
		// keyboard over the grid.
		if (autofocus) filters?.focus({ preventScroll: true });
	});
</script>

<div
	bind:this={filters}
	role="group"
	aria-label="Track filters"
	tabindex="-1"
	class="flex flex-wrap items-center gap-2 outline-none"
>
	<div class="relative w-full flex-1 sm:w-auto sm:min-w-48">
		<SearchIcon
			class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
		/>
		<Input
			class="h-9 pl-8"
			aria-label="Search tracks"
			placeholder="Search tracks..."
			value={term}
			oninput={(event) => (term = event.currentTarget.value)}
		/>
	</div>
	{@render trailing?.()}
</div>
{#if locations.length > 1}
	<ToggleGroup.Root
		type="single"
		value={locationFilter}
		variant="outline"
		size="lg"
		class="w-full flex-wrap"
		aria-label="Filter tracks by location"
		onValueChange={(value) => (locationFilter = value || 'all')}
	>
		<ToggleGroup.Item value="all" class="flex-initial sm:flex-auto"
			>All</ToggleGroup.Item
		>
		{#each locations as location (location.code)}
			<ToggleGroup.Item value={location.code} class="flex-initial sm:flex-auto"
				>{location.name}</ToggleGroup.Item
			>
		{/each}
	</ToggleGroup.Root>
{/if}
<div class="-mx-1 overflow-y-auto px-1 {height}">
	{#if failed}
		<div class="space-y-3 py-12 text-center text-sm">
			<p class="text-destructive">Could not load tracks. Please try again.</p>
			<button
				type="button"
				class="cursor-pointer underline"
				onclick={() => retry++}>Try again</button
			>
		</div>
	{:else if !tracks}
		<p role="status" class="py-12 text-center text-sm text-muted-foreground">
			Loading tracks...
		</p>
	{:else if shown.length}
		<ul class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
			{#each shown as option (option.code)}
				<li>
					<TrackCard
						track={option}
						selected={option.code === selected?.code}
						{onSelect}
					/>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="py-12 text-center text-sm text-muted-foreground">
			{tracks.length
				? 'No tracks match that search.'
				: carrying
					? `This era pairs no tracks with ${carrying.name}.`
					: 'This era offers no combinations yet.'}
		</p>
	{/if}
</div>
