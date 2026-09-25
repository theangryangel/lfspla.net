<script lang="ts">
	import type { Snippet } from 'svelte';
	import CarIcon from '@lucide/svelte/icons/car';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import XIcon from '@lucide/svelte/icons/x';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import TrackPicker from '$lib/components/app/TrackPicker.svelte';
	import VehiclePicker from '$lib/components/app/VehiclePicker.svelte';
	import Thumbnail from '$lib/components/app/Thumbnail.svelte';
	import { setCombinationPicker, type PickerRequest } from '$lib/picker.js';
	import type { TrackSummary, VehicleSummary } from '$lib/api.js';

	/**
	 * The era's single combination dialog.
	 *
	 * Triggers scattered across the era ask for it through the context this
	 * publishes, so only one dialog is ever mounted and only one place decides
	 * where a chosen combination leads.
	 */
	let {
		era,
		onSelect,
		children,
	}: {
		era: string;
		onSelect: (track: string, vehicle: string) => void;
		children: Snippet;
	} = $props();

	/** The half settled on first, which then narrows the other. */
	type Half =
		| { kind: 'track'; of: TrackSummary }
		| { kind: 'vehicle'; of: VehicleSummary };

	let dialogContent = $state<HTMLDivElement | null>(null);
	let visible = $state(false);
	// The grid in view. Neither half is the natural first step - people arrive
	// wanting a track as often as they arrive wanting a vehicle - so with nothing
	// to go on the dialog asks where to start instead.
	let showing = $state<'tracks' | 'vehicles' | null>(null);
	// The half that narrows the grid, so whatever is picked there is valid.
	let holding = $state<Half | null>(null);
	// Whether that half was picked in here, which decides what letting go means.
	let stepped = $state(false);
	// What the opener is changing, marked in the grids. The dialog holds its own
	// copy so a navigation underneath it cannot move the ground mid-edit.
	let editing = $state<Omit<PickerRequest, 'mode' | 'from'>>({});
	// Only a step the reader moved to takes focus; the first is where the dialog
	// already put them.
	let moved = $state(false);
	let opener: HTMLElement | null = null;

	const START =
		'flex cursor-pointer items-center gap-3 rounded-lg border p-4 text-left transition-colors hover:border-ring hover:bg-accent/40 focus-visible:outline-2 focus-visible:outline-ring';

	const title = $derived.by(() => {
		if (!showing) return 'Choose a combination';
		const half = showing === 'tracks' ? 'track' : 'vehicle';
		// A held half is what the grid is narrowed to, so the title says what is
		// on offer rather than leaving it to the control below.
		return holding
			? `Choose a ${half} for ${holding.of.name}`
			: `Choose a ${half}`;
	});
	// The track a vehicle grid is scoped to. Without one it offers the era's
	// whole catalogue, and the track follows from the vehicle instead.
	const scoped = $derived(holding?.kind === 'track' ? holding.of : null);
	// The vehicle a track grid keeps, narrowing it to the tracks that offer it.
	const carrying = $derived(holding?.kind === 'vehicle' ? holding.of : null);

	export function open(request: PickerRequest = {}) {
		const { mode = 'combination', track = null, vehicle = null } = request;
		editing = { track, vehicle };
		// Changing one half alone needs the other to narrow by, so a request
		// missing either asks where to start. Browsing is a fresh start rather
		// than a step in one, so it holds nothing however deep the page behind it
		// is; the combination in view is still marked in both grids.
		if (track && vehicle && mode === 'track') {
			showing = 'tracks';
			holding = { kind: 'vehicle', of: vehicle };
		} else if (track && vehicle && mode === 'vehicle') {
			showing = 'vehicles';
			holding = { kind: 'track', of: track };
		} else {
			showing = null;
			holding = null;
		}
		stepped = false;
		moved = false;
		opener = request.from ?? null;
		visible = true;
	}

	setCombinationPicker({ open });

	function choose(trackCode: string, vehicleCode: string) {
		visible = false;
		onSelect(trackCode, vehicleCode);
	}

	/** Settles one half here and moves on to the other. */
	function hold(half: Half) {
		holding = half;
		stepped = true;
		showing = half.kind === 'track' ? 'vehicles' : 'tracks';
		moved = true;
	}

	function pickTrack(picked: TrackSummary) {
		// A held vehicle narrows the grid to the tracks offering it, so anything
		// picked there completes the pair outright.
		if (holding?.kind === 'vehicle')
			return choose(picked.code, holding.of.code);
		hold({ kind: 'track', of: picked });
	}

	function pickVehicle(picked: VehicleSummary) {
		if (holding?.kind === 'track') return choose(holding.of.code, picked.code);
		hold({ kind: 'vehicle', of: picked });
	}

	function release() {
		// A half settled on here steps back to the grid it was picked from. One
		// handed over by the opener simply stops narrowing, widening the grid in
		// view to the era's whole catalogue.
		if (stepped) showing = holding?.kind === 'track' ? 'tracks' : 'vehicles';
		holding = null;
		stepped = false;
		moved = true;
	}

	function restoreFocus(event: Event) {
		if (!opener) return;
		event.preventDefault();
		opener.focus({ preventScroll: true });
		opener = null;
	}

	// The next opener says what is being changed, so a closed dialog holds no
	// opinion left over from the last one.
	$effect(() => {
		if (visible) return;
		showing = null;
		holding = null;
		stepped = false;
		moved = false;
	});
</script>

{#snippet heldControl()}
	{#if holding}
		{@const outcome = stepped
			? `Change ${holding.kind} instead`
			: `Change ${holding.kind} too`}
		<span class="shrink-0 text-sm text-muted-foreground">for</span>
		<div
			class="flex h-9 flex-1 items-center gap-2 rounded-md border bg-secondary pr-1 pl-2 text-sm sm:flex-none"
		>
			<Thumbnail
				kind={holding.kind}
				code={holding.of.code}
				src={holding.kind === 'vehicle' ? holding.of.image_url : null}
				class="w-10 shrink-0 rounded-sm ring-1 ring-border"
			/>
			<span
				class="flex-1 truncate text-left font-medium sm:max-w-40 sm:flex-none"
				>{holding.of.name}</span
			>
			<button
				type="button"
				title={outcome}
				aria-label="{holding.of.name}: {outcome}."
				onclick={release}
				class="shrink-0 cursor-pointer rounded-sm p-1 text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"
			>
				<XIcon class="size-4" />
			</button>
		</div>
	{/if}
{/snippet}

{@render children()}

<Dialog.Root bind:open={visible}>
	<Dialog.Content
		bind:ref={dialogContent}
		tabindex={-1}
		class="gap-3 sm:max-w-3xl"
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			dialogContent?.focus({ preventScroll: true });
		}}
		onCloseAutoFocus={restoreFocus}
	>
		<Dialog.Header class="pr-8">
			<Dialog.Title>{title}</Dialog.Title>
		</Dialog.Header>
		{#if !showing}
			<div class="grid gap-3 py-2 sm:grid-cols-2">
				<button
					type="button"
					onclick={() => ((showing = 'tracks'), (moved = true))}
					class={START}
				>
					<MapPinIcon class="size-6 shrink-0 text-muted-foreground" />
					<span class="min-w-0">
						<span class="block font-semibold">Start with a track</span>
						<span class="block text-sm text-muted-foreground"
							>Then one of the vehicles it pairs with.</span
						>
					</span>
				</button>
				<button
					type="button"
					onclick={() => ((showing = 'vehicles'), (moved = true))}
					class={START}
				>
					<CarIcon class="size-6 shrink-0 text-muted-foreground" />
					<span class="min-w-0">
						<span class="block font-semibold">Start with a vehicle</span>
						<span class="block text-sm text-muted-foreground"
							>Then one of the tracks that offer it.</span
						>
					</span>
				</button>
			</div>
		{:else if showing === 'vehicles'}
			<VehiclePicker
				{era}
				track={scoped}
				selected={!scoped || scoped.code === editing.track?.code
					? editing.vehicle
					: null}
				autofocus={moved}
				trailing={holding ? heldControl : undefined}
				onSelect={pickVehicle}
			/>
		{:else}
			<TrackPicker
				{era}
				selected={editing.track}
				{carrying}
				autofocus={moved}
				trailing={holding ? heldControl : undefined}
				onSelect={pickTrack}
			/>
		{/if}
	</Dialog.Content>
</Dialog.Root>
