<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import CheckIcon from '@lucide/svelte/icons/check';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import Thumbnail from '$lib/components/app/Thumbnail.svelte';
	import { getEraTracks } from '$lib/combinations.js';
	import { CARD } from '$lib/components/app/picker.js';
	import type {
		TrackLocation,
		TrackLocationCode,
		TrackSummary,
		VehicleSummary,
	} from '$lib/api.js';

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
	let locationFilter = $state<TrackLocationCode | 'all'>('all');
	let filters = $state<HTMLDivElement | null>(null);
	let tracks = $state<TrackSummary[] | null>(null);
	let failed = $state(false);
	let retry = $state(0);

	const contains = (text: string) =>
		text.toLowerCase().includes(term.trim().toLowerCase());
	const locationOrder: TrackLocationCode[] = [
		'BL',
		'SO',
		'FE',
		'KY',
		'AS',
		'WE',
		'RO',
		'OTHER',
	];
	const locations = $derived(
		locationOrder
			.map(
				(code) =>
					tracks?.find((track) => track.location.code === code)?.location,
			)
			.filter((location): location is TrackLocation => location !== undefined),
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
		getEraTracks(era, vehicle)
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
		onValueChange={(value) =>
			(locationFilter = (value || 'all') as TrackLocationCode | 'all')}
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
					<button
						type="button"
						aria-current={option.code === selected?.code ? 'true' : undefined}
						class={CARD}
						onclick={() => onSelect(option)}
					>
						<Thumbnail kind="track" code={option.code} />
						<span class="flex items-center gap-1.5 p-2">
							<span class="min-w-0 flex-1">
								<span class="block truncate text-sm font-medium"
									>{option.name}</span
								>
								<span class="block font-mono text-xs text-muted-foreground"
									>{option.code}</span
								>
							</span>
							{#if option.reverse}
								<Badge variant="outline">Rev</Badge>
							{/if}
							{#if option.code === selected?.code}
								<CheckIcon class="size-4 shrink-0 text-primary" />
							{/if}
						</span>
					</button>
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
