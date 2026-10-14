<script lang="ts">
	import type { TrackSummary } from '$lib/api.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import CheckIcon from '@lucide/svelte/icons/check';
	import Thumbnail from './Thumbnail.svelte';
	import { CARD } from './picker.js';
	import { cn } from '$lib/utils.js';

	// Supplying onSelect renders the compact picker button instead of a catalogue card.
	let {
		track,
		onSelect,
		selected = false,
	}: {
		track: TrackSummary;
		onSelect?: (track: TrackSummary) => void;
		selected?: boolean;
	} = $props();
</script>

{#snippet content()}
	<Thumbnail kind="track" code={track.code} />
	<div class="flex items-center gap-1.5 p-2">
		<div class="min-w-0 flex-1">
			<svelte:element
				this={onSelect ? 'span' : 'h2'}
				class="block truncate text-sm font-medium">{track.name}</svelte:element
			>
			<div class="font-mono text-xs text-muted-foreground">{track.code}</div>
			{#if !onSelect}<div class="truncate text-xs text-muted-foreground">
					{track.location.name}
				</div>{/if}
		</div>
		<div
			class={onSelect ? 'contents' : 'flex shrink-0 flex-col items-end gap-1'}
		>
			{#if track.reverse}<Badge variant="outline">Rev</Badge>{/if}
			{#if !onSelect && track.open_configuration}<Badge variant="secondary"
					>Open</Badge
				>{/if}
			{#if onSelect && selected}<CheckIcon
					class="size-4 shrink-0 text-primary"
					aria-hidden="true"
				/>{/if}
		</div>
	</div>
{/snippet}

{#if onSelect}
	<button
		type="button"
		class={CARD}
		aria-current={selected ? 'true' : undefined}
		onclick={() => onSelect?.(track)}
	>
		{@render content()}
	</button>
{:else}
	<div class={cn(CARD, 'cursor-default')} title={track.name}>
		{@render content()}
	</div>
{/if}
