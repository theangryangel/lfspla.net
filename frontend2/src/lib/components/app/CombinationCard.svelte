<script lang="ts">
	import PageIdentity from './PageIdentity.svelte';
	import RecordHighlight from './RecordHighlight.svelte';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import Thumbnail from '$lib/components/app/Thumbnail.svelte';
	import { useCombinationPicker } from '$lib/picker.js';
	import Flag from './Flag.svelte';
	import { lapTime } from '$lib/format.js';
	import type { Hotlap, TrackSummary, VehicleSummary } from '$lib/api.js';

	let {
		track = null,
		vehicle = null,
		worldRecord = null,
	}: {
		track?: TrackSummary | null;
		vehicle?: VehicleSummary | null;
		worldRecord?: Hotlap | null;
	} = $props();
	const picker = useCombinationPicker();
	const selectorClass =
		'flex w-full min-w-0 cursor-pointer items-center gap-3 rounded-sm py-1 text-left transition-colors hover:bg-muted/50 focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-4';
</script>

{#snippet selector(
	kind: 'track' | 'vehicle',
	code: string | undefined,
	name: string | undefined,
	image: string | null,
)}
	<span class="flex min-w-0 items-center gap-3 py-2">
		{#if code}
			<Thumbnail src={image} class="w-16 shrink-0 bg-transparent sm:w-24" />
		{/if}
		<span class="min-w-0 flex-1">
			<button
				type="button"
				title={name || undefined}
				aria-label={name ? `Change the ${kind}: ${name}` : `Choose a ${kind}`}
				aria-haspopup="dialog"
				onclick={(event) =>
					picker.open({
						mode: kind,
						track,
						vehicle,
						from: event.currentTarget,
					})}
				class={selectorClass}
			>
				<span class="racing-page-title break-words"
					>{name ?? `Choose a ${kind}`}</span
				>
				<ChevronDownIcon
					class="size-4 shrink-0 text-primary"
					aria-hidden="true"
				/>
			</button>
			{#if code}<span class="block text-sm text-muted-foreground">{code}</span
				>{/if}
		</span>
	</span>
{/snippet}

<PageIdentity label="Chart identity">
	<div
		class="grid min-w-0 content-center items-center gap-x-8 gap-y-2 lg:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)]"
	>
		<h1 class="min-w-0">
			{@render selector(
				'track',
				track?.code,
				track?.name,
				track?.image?.url ?? track?.outline?.url ?? null,
			)}
		</h1>
		<div
			class="min-w-0 border-t pt-1 lg:border-t-0 lg:border-l lg:pt-0 lg:pl-6"
		>
			{@render selector(
				'vehicle',
				vehicle?.code,
				vehicle?.name,
				vehicle?.image?.url ?? null,
			)}
		</div>
	</div>
	{#snippet highlight()}
		{#if worldRecord !== null}
			<RecordHighlight title="World record" variant="purple">
				<p
					class="relative text-right whitespace-nowrap font-mono text-2xl font-bold tabular-nums lg:text-3xl"
				>
					{lapTime(worldRecord.lap_time_ms)}
				</p>
				<div
					class="relative flex flex-wrap items-center justify-end gap-2 text-sm"
				>
					<a
						class="flex min-w-0 items-center gap-2 font-medium hover:underline"
						href={`/drivers/${encodeURIComponent(worldRecord.player.lfs_username)}`}
					>
						<Flag
							code={worldRecord.player.flag_code}
							fallback={worldRecord.player.country_code}
						/>
						<span class="break-words">{worldRecord.player.display_name}</span>
					</a>
				</div>
			</RecordHighlight>
		{/if}
	{/snippet}
</PageIdentity>
