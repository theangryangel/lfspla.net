<script lang="ts">
	import type { VehicleSummary } from '$lib/api.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import Thumbnail from './Thumbnail.svelte';
	import { CARD } from './picker.js';
	import { cn } from '$lib/utils.js';

	// Supplying onSelect renders the compact picker button instead of a catalogue card.
	let {
		vehicle,
		onSelect,
		selected = false,
	}: {
		vehicle: VehicleSummary;
		onSelect?: (vehicle: VehicleSummary) => void;
		selected?: boolean;
	} = $props();
	const mod = $derived(/^[0-9A-F]{6}$/i.test(vehicle.code));
</script>

{#snippet content()}
	<Thumbnail kind="vehicle" code={vehicle.code} src={vehicle.image?.url} />
	<div class="flex items-center gap-1.5 p-2">
		<div class="min-w-0 flex-1">
			<svelte:element
				this={onSelect ? 'span' : 'h2'}
				class="block truncate text-sm font-medium"
				>{vehicle.name}</svelte:element
			>
			<div class="text-xs text-muted-foreground">
				<span class="font-mono">{vehicle.code}</span>
				{#if !onSelect}<span> · {mod ? 'Mod' : 'Standard'}</span>{/if}
			</div>
		</div>
		<Badge variant="secondary">{vehicle.license}</Badge>
	</div>
{/snippet}

{#if onSelect}
	<button
		type="button"
		class={CARD}
		aria-current={selected ? 'true' : undefined}
		onclick={() => onSelect?.(vehicle)}
	>
		{@render content()}
	</button>
{:else if mod}
	<a
		href="https://www.lfs.net/files/vehmods/{vehicle.code}"
		class={cn(CARD, 'block')}
		title={vehicle.name}
	>
		{@render content()}
	</a>
{:else}
	<div class={cn(CARD, 'cursor-default')} title={vehicle.name}>
		{@render content()}
	</div>
{/if}
