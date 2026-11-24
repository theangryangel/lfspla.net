<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import HotlapDisciplineBadge from './HotlapDisciplineBadge.svelte';
	import ChartContributions from './ChartContributions.svelte';
	import PageIdentity from './PageIdentity.svelte';
	import RecordHighlight from './RecordHighlight.svelte';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import Thumbnail from '#lib/components/app/Thumbnail.svelte';
	import { useCombinationPicker } from '#lib/picker.js';
	import Flag from './Flag.svelte';
	import { lapTime } from '#lib/format.js';
	import type {
		EraSummary,
		RankingContribution,
		Hotlap,
		TrackSummary,
		VehicleSummary,
	} from '#lib/api.js';

	let {
		track = null,
		vehicle = null,
		worldRecord = null,
		era,
		contributions = [],
	}: {
		track?: TrackSummary | null;
		vehicle?: VehicleSummary | null;
		worldRecord?: Hotlap | null;
		era?: EraSummary;
		contributions?: RankingContribution[];
	} = $props();
	const picker = useCombinationPicker();
	const disciplines = $derived(
		(['circuit', 'drag'] as const).filter((discipline) =>
			era?.rankings.some(
				(ranking) =>
					ranking.discipline === discipline &&
					contributions.some((contribution) => contribution.id === ranking.id),
			),
		),
	);
</script>

{#snippet selector(
	kind: 'track' | 'vehicle',
	code: string | undefined,
	name: string | undefined,
	image: string | null,
)}
	<Button
		variant="outline"
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
		class="h-auto w-full min-w-0 justify-start gap-3 whitespace-normal px-3 py-3 text-left"
	>
		{#if code}<Thumbnail
				src={image}
				class="w-16 shrink-0 bg-transparent sm:w-20"
			/>{/if}
		<span class="min-w-0 flex-1">
			<span class="racing-page-title block break-words"
				>{name ?? `Choose a ${kind}`}</span
			>
			{#if code}<span
					class="mt-1 block text-sm font-normal text-muted-foreground"
					>{code}</span
				>{/if}
		</span>
		<ChevronDownIcon
			class="size-4 shrink-0 text-black dark:text-white"
			aria-hidden="true"
		/>
	</Button>
{/snippet}

<PageIdentity label="Chart identity">
	<div
		class="grid min-w-0 content-center items-center gap-x-5 gap-y-4 lg:grid-cols-2 xl:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)_minmax(0,0.9fr)]"
	>
		<h1 class="min-w-0">
			{@render selector(
				'track',
				track?.code,
				track?.name,
				track?.image?.url ?? track?.outline?.url ?? null,
			)}
		</h1>
		<div class="min-w-0">
			{@render selector(
				'vehicle',
				vehicle?.code,
				vehicle?.name,
				vehicle?.image?.url ?? null,
			)}
		</div>
		{#if era}
			<div
				class="min-w-0 border-t pt-3 lg:col-span-2 xl:col-span-1 xl:border-t-0 xl:border-l xl:pt-0 xl:pl-5"
			>
				{#if disciplines.length}
					<div class="mb-3 flex flex-wrap gap-2" aria-label="Chart disciplines">
						{#each disciplines as discipline (discipline)}
							<HotlapDisciplineBadge {discipline} />
						{/each}
					</div>
				{/if}

				<ChartContributions {era} {contributions} />
			</div>
		{/if}
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
