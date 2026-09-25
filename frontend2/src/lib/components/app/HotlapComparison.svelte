<script lang="ts">
	import type { BestHotlapResponse } from '$lib/api.js';
	import { intermediateSplits, sectorTimes } from '$lib/hotlap-comparison.js';
	import { delta, lapTime } from '$lib/format.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { useComparison } from '$lib/compare.svelte.js';
	import { mergeProps } from 'bits-ui';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import GitCompareArrows from '@lucide/svelte/icons/git-compare-arrows';
	import Timer from '@lucide/svelte/icons/timer';
	import Minimize2 from '@lucide/svelte/icons/minimize-2';
	import Maximize2 from '@lucide/svelte/icons/maximize-2';
	import Flag from './Flag.svelte';
	import X from '@lucide/svelte/icons/x';
	let {
		laps,
		onRemove,
		onClear,
	}: {
		laps: BestHotlapResponse[];
		onRemove: (lap: BestHotlapResponse) => void;
		onClear: () => void;
	} = $props();
	let minimized = $state(false);
	const comparison = useComparison();
	const splits = $derived(laps.map(sectorTimes));
	const absolute = $derived(
		laps.map((lap) => [...intermediateSplits(lap), lap.lap_time_ms]),
	);
	const rows = $derived([
		...Array.from(
			{ length: Math.max(0, ...splits.map((s) => s.length)) },
			(_, i) => ({
				label: `Split ${i + 1}`,
				relative: laps.map((_, j) => splits[j][i]),
				absolute: laps.map((_, j) => absolute[j][i]),
			}),
		),
		{
			label: 'Lap time',
			relative: [] as number[],
			absolute: laps.map((lap) => lap.lap_time_ms),
		},
	]);
	function relativeColor(time: number | undefined, other: number | undefined) {
		if (time === undefined || other === undefined || time === other) return '';
		return time < other
			? 'text-green-700 dark:text-green-400'
			: 'text-red-700 dark:text-red-400';
	}
	function signedDelta(ms: number) {
		return ms === 0 ? `\u00a0${lapTime(0)}` : delta(ms);
	}
</script>

<section
	aria-label="Lap comparison"
	class="overflow-hidden rounded-lg border border-border bg-popover/95 text-popover-foreground shadow-lg backdrop-blur-xl"
>
	<div class="flex items-center justify-between gap-3 px-3 py-2 sm:px-4">
		<h2 class="flex items-center gap-2 text-sm font-semibold">
			<Timer class="size-4 text-primary" aria-hidden="true" />Lap comparison
		</h2>
		<div class="flex items-center gap-1">
			{#if laps.length === 2}
				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<Button
								{...mergeProps(props, {
									onclick: () => comparison.seed(laps.map((lap) => lap.player)),
								})}
								variant="ghost"
								size="icon-sm"
								class="size-7"
								aria-label="Compare these drivers"
							>
								<GitCompareArrows class="size-3.5" aria-hidden="true" />
							</Button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>Compare these drivers</Tooltip.Content>
				</Tooltip.Root>
			{/if}
			<Button
				variant="ghost"
				size="icon-sm"
				class="size-7 aria-expanded:bg-transparent"
				onclick={() => (minimized = !minimized)}
				aria-expanded={!minimized}
				aria-label={minimized
					? 'Expand lap comparison'
					: 'Minimize lap comparison'}
			>
				{#if minimized}<Maximize2
						class="size-3.5"
						aria-hidden="true"
					/>{:else}<Minimize2 class="size-3.5" aria-hidden="true" />{/if}
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				class="size-7"
				onclick={onClear}
				aria-label="Clear lap comparison"
				><X class="size-3.5" aria-hidden="true" /></Button
			>
		</div>
	</div>
	{#if !minimized}
		<div class="overflow-x-auto">
			<table class="w-full min-w-[22rem] table-fixed text-xs">
				<caption class="sr-only"
					>Relative times measure each segment; absolute times measure elapsed
					time from the start. Differences are first minus second relative time.
					Faster relative times are green; slower times are red.</caption
				>
				<thead>
					<tr class="border-b border-border">
						{#each [0, 1] as index}
							{#if index === 1}<th
									scope="col"
									class="pb-2 text-center text-xs font-medium text-muted-foreground"
									title="First selected relative time minus second selected relative time"
									>Diff</th
								>{/if}
							<th scope="colgroup" colspan="2" class="px-1 pb-2 font-normal">
								{#if laps[index]}
									<div
										class="mx-auto flex w-fit max-w-full items-center gap-1 rounded-md border border-border bg-background/60 py-1 pr-1 pl-2 sm:gap-2"
									>
										<Flag
											code={laps[index].player.country_code}
											class="hidden sm:inline-flex"
										/>
										<span
											class="min-w-0 truncate"
											title={laps[index].player.display_name}
											>{laps[index].player.display_name}</span
										>
										<Button
											variant="ghost"
											size="icon-sm"
											class="size-6 shrink-0"
											onclick={() => onRemove(laps[index])}
											aria-label="Remove {laps[index].player
												.display_name}'s lap"
											><X class="size-3.5" aria-hidden="true" /></Button
										>
									</div>
								{:else}<span class="text-xs text-muted-foreground"
										>Select a second lap</span
									>{/if}
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each rows as row, index}
						{@const a = row.absolute[0]}
						{@const b = row.absolute[1]}
						{@const left = row.relative[0]}
						{@const right = row.relative[1]}
						{@const difference =
							a !== undefined && b !== undefined
								? index === rows.length - 1
									? a - b
									: left !== undefined && right !== undefined
										? left - right
										: undefined
								: undefined}
						<tr
							class:border-t-2={index === rows.length - 1}
							class="border-b border-border/60 last:border-0 last:bg-background/40 last:font-semibold"
						>
							<td
								class="py-2 text-center font-mono tabular-nums {relativeColor(
									a,
									b,
								)}"
								title={a !== undefined && b !== undefined
									? a === b
										? 'Equal elapsed times'
										: a < b
											? 'Faster elapsed time'
											: 'Slower elapsed time'
									: undefined}>{lapTime(a)}</td
							>
							<td
								class="py-2 text-center font-mono tabular-nums {relativeColor(
									left,
									right,
								)}"
								title={left !== undefined && right !== undefined
									? left === right
										? 'Equal segment times'
										: left < right
											? 'Faster segment'
											: 'Slower segment'
									: undefined}>{lapTime(left)}</td
							>
							<th scope="row" class="py-2 text-center font-normal">
								<span
									class="block whitespace-nowrap font-mono tabular-nums"
									title={index === rows.length - 1
										? 'First selected absolute time minus second selected absolute time'
										: 'First selected relative time minus second selected relative time'}
								>
									{difference !== undefined ? signedDelta(difference) : '-'}
								</span>
							</th>
							<td
								class="py-2 text-center font-mono tabular-nums {relativeColor(
									right,
									left,
								)}"
								title={left !== undefined && right !== undefined
									? left === right
										? 'Equal segment times'
										: right < left
											? 'Faster segment'
											: 'Slower segment'
									: undefined}>{lapTime(right)}</td
							>
							<td
								class="py-2 text-center font-mono tabular-nums {relativeColor(
									b,
									a,
								)}"
								title={a !== undefined && b !== undefined
									? a === b
										? 'Equal elapsed times'
										: b < a
											? 'Faster elapsed time'
											: 'Slower elapsed time'
									: undefined}>{lapTime(b)}</td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</section>
