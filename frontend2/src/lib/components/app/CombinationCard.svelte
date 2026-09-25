<script lang="ts">
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import Thumbnail from '$lib/components/app/Thumbnail.svelte';
	import { useCombinationPicker } from '$lib/picker.js';
	import type { TrackSummary, VehicleSummary } from '$lib/api.js';

	/** The combination in view, with each half its own way into the picker. */
	let {
		track = null,
		vehicle = null,
	}: {
		track?: TrackSummary | null;
		vehicle?: VehicleSummary | null;
	} = $props();

	const picker = useCombinationPicker();
	// Wide enough for the long names the catalogue and mods actually carry, with
	// a second line behind that rather than a cut-off word.
	const BLOB =
		'flex min-w-0 flex-1 cursor-pointer items-center gap-3 rounded-lg border p-2 text-left transition-colors hover:border-ring hover:bg-accent/40 focus-visible:outline-2 focus-visible:outline-ring sm:w-72 sm:flex-none';
</script>

{#snippet blob(
	kind: 'track' | 'vehicle',
	code: string | undefined,
	name: string | undefined,
	image: string | null,
)}
	<button
		type="button"
		title={name || undefined}
		aria-label={name ? `Change the ${kind}: ${name}` : `Choose a ${kind}`}
		onclick={(event) =>
			picker.open({
				mode: kind,
				track,
				vehicle,
				from: event.currentTarget,
			})}
		class={BLOB}
	>
		{#if code}
			<Thumbnail
				{kind}
				{code}
				src={image}
				class="w-20 shrink-0 rounded-md ring-1 ring-border"
			/>
		{:else}
			<span class="aspect-16/10 w-20 shrink-0 rounded-md border border-dashed"
			></span>
		{/if}
		<span class="min-w-0 flex-1">
			<span class="line-clamp-2 text-sm leading-tight font-semibold"
				>{name ?? `Choose a ${kind}`}</span
			>
			<span class="block font-mono text-xs text-muted-foreground"
				>{code ?? kind}</span
			>
		</span>
		<PencilIcon class="size-4 shrink-0 opacity-50" />
	</button>
{/snippet}

<div class="flex w-full flex-wrap items-center gap-4 sm:w-auto">
	{@render blob('track', track?.code, track?.name, null)}
	{@render blob(
		'vehicle',
		vehicle?.code,
		vehicle?.name,
		vehicle?.image_url ?? null,
	)}
</div>
