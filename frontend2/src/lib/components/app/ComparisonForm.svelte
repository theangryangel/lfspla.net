<script lang="ts">
	import { goto } from '$app/navigation';
	import {
		getList,
		query,
		type EraSummary,
		type TrackSummary,
		type VehicleSummary,
	} from '$lib/api.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import Panel from './Panel.svelte';
	import SearchSelect, { type SearchOption } from './SearchSelect.svelte';

	let {
		eras,
		initial,
	}: {
		eras: EraSummary[];
		initial: {
			left: string;
			right: string;
			era: string;
			track: string;
			vehicle: string;
		};
	} = $props();
	let left = $derived(initial.left);
	let right = $derived(initial.right);
	let era = $derived(initial.era);
	let track = $derived(initial.track);
	let vehicle = $derived(initial.vehicle);
	let tracks = $state<TrackSummary[]>([]);
	let vehicles = $state<VehicleSummary[]>([]);
	let vehicleLabels = $state<Record<string, string>>({});
	let trackProblem = $state('');
	let vehicleProblem = $state('');
	let pending = $state(false);
	let problem = $state('');
	const eraOptions = $derived(
		eras.map((item) => ({ value: item.id, label: item.title })),
	);
	const trackOptions = $derived([
		{ value: '', label: 'All tracks' },
		...tracks.map((item) => ({
			value: item.code,
			label: item.name,
			hint: item.code,
		})),
	]);
	function vehicleOptions(items: VehicleSummary[]): SearchOption[] {
		return [
			{ value: '', label: 'All vehicles' },
			...items.map((item) => ({
				value: item.code,
				label: item.name,
				hint: item.code,
			})),
		];
	}

	$effect(() => {
		const selectedEra = era;
		let active = true;
		tracks = [];
		trackProblem = '';
		if (selectedEra) {
			getList<TrackSummary>(
				fetch,
				`/api/v1/eras/${encodeURIComponent(selectedEra)}/tracks`,
			)
				.then((items) => {
					if (active) tracks = items;
				})
				.catch(() => {
					if (active) trackProblem = 'Could not load tracks. Please try again.';
				});
		}
		return () => {
			active = false;
		};
	});

	$effect(() => {
		const selectedEra = era;
		const selectedTrack = track;
		let active = true;
		vehicles = [];
		vehicleLabels = {};
		vehicleProblem = '';
		if (selectedEra) {
			getList<VehicleSummary>(
				fetch,
				`/api/v1/eras/${encodeURIComponent(selectedEra)}/vehicles` +
					query({ track: selectedTrack, limit: 100 }),
			)
				.then((items) => {
					if (active) vehicles = items;
				})
				.catch(() => {
					if (active)
						vehicleProblem = 'Could not load vehicles. Please try again.';
				});
		}
		return () => {
			active = false;
		};
	});

	async function searchVehicles(term: string) {
		const selectedEra = era;
		const selectedTrack = track;
		const items = await getList<VehicleSummary>(
			fetch,
			`/api/v1/eras/${encodeURIComponent(selectedEra)}/vehicles` +
				query({ track: selectedTrack, q: term, limit: 100 }),
		);
		if (era === selectedEra && track === selectedTrack) {
			vehicleLabels = {
				...vehicleLabels,
				...Object.fromEntries(items.map((item) => [item.code, item.name])),
			};
		}
		return vehicleOptions(items);
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !era) return;
		pending = true;
		problem = '';
		try {
			await goto(
				'/compare' +
					query({
						left: left.trim(),
						right: right.trim(),
						era,
						track,
						vehicle,
					}),
			);
		} catch {
			problem = 'Could not load the comparison. Please try again.';
		} finally {
			pending = false;
		}
	}
</script>

<Panel
	title="Choose drivers"
	description="Compare personal bests in an era, with optional track and vehicle filters."
>
	<form action="/compare" method="GET" class="space-y-4" onsubmit={submit}>
		<div class="grid gap-4 sm:grid-cols-2 lg:max-w-2xl">
			<label class="grid gap-2 text-sm font-medium"
				>First LFS username<Input
					name="left"
					required
					bind:value={left}
				/></label
			>
			<label class="grid gap-2 text-sm font-medium"
				>Second LFS username<Input
					name="right"
					required
					bind:value={right}
				/></label
			>
		</div>
		<div class="flex flex-wrap items-end gap-4 border-t pt-4">
			<div class="grid gap-2">
				<span class="text-sm font-medium">Era</span>
				<SearchSelect
					label="Era"
					value={era}
					options={eraOptions}
					placeholder="Search eras..."
					onValueChange={(value) => {
						era = value;
						track = '';
						vehicle = '';
					}}
				/>
			</div>
			<div class="grid gap-2">
				<span class="text-sm font-medium">Track</span>
				<SearchSelect
					label="Track"
					value={track}
					selectedLabel={tracks.find((item) => item.code === track)?.name ??
						track}
					options={trackOptions}
					placeholder="Search tracks..."
					onValueChange={(value) => {
						track = value;
						vehicle = '';
					}}
				/>
			</div>
			<div class="grid gap-2">
				<span class="text-sm font-medium">Vehicle</span>
				{#key `${era}/${track}`}
					<SearchSelect
						label="Vehicle"
						value={vehicle}
						selectedLabel={vehicleLabels[vehicle] ??
							vehicles.find((item) => item.code === vehicle)?.name ??
							vehicle}
						options={vehicleOptions(vehicles)}
						search={searchVehicles}
						placeholder="Search vehicles..."
						onValueChange={(value) => {
							vehicle = value;
						}}
					/>
				{/key}
			</div>
			<input type="hidden" name="era" value={era} />
			<input type="hidden" name="track" value={track} />
			<input type="hidden" name="vehicle" value={vehicle} />
			<Button type="submit" disabled={pending || !era}
				>{pending ? 'Comparing...' : 'Compare'}</Button
			>
			{#if track || vehicle}
				<Button
					variant="ghost"
					onclick={() => {
						track = '';
						vehicle = '';
					}}>Clear filters</Button
				>
			{/if}
		</div>
		{#if trackProblem || vehicleProblem || problem}
			<p role="alert" class="text-sm text-destructive">
				{problem || trackProblem || vehicleProblem}
			</p>
		{/if}
	</form>
</Panel>
