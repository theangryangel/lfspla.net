<script lang="ts">
	import type { PageProps } from './$types';
	import { useApi, apiErrorMessage } from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import { goto, invalidate } from '$app/navigation';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import RecordHighlight, {
		recordHighlightValueClass,
	} from '#lib/components/app/RecordHighlight.svelte';
	import RecordHighlights from '#lib/components/app/RecordHighlights.svelte';
	import Breadcrumbs from '#lib/components/app/Breadcrumbs.svelte';
	import DateTime from '#lib/components/app/DateTime.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
	import Thumbnail from '#lib/components/app/Thumbnail.svelte';
	import TrackSectorMap from '#lib/components/app/TrackSectorMap.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import ReportHotlap from '#lib/components/app/ReportHotlap.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import ConfirmDialog from '#lib/components/app/ConfirmDialog.svelte';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import HotlapComments from '#lib/components/app/HotlapComments.svelte';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import {
		controlTags,
		delta,
		hotlapStates,
		lapTime,
		steeringLabels,
	} from '#lib/format.js';
	import {
		compareSectors,
		intermediateSplits,
		sectorTimes,
		sectorColour,
	} from '#lib/hotlap-comparison.js';
	let { data }: PageProps = $props();
	const api = useApi();
	const session = useSession();
	const hotlap = $derived(data.hotlap);
	const owner = $derived(
		session.signedIn &&
			hotlap.player.id === session.player?.id &&
			Boolean(hotlap.submission),
	);
	const status = $derived(hotlapStates[hotlap.state]);
	const chartHref = $derived(
		`/hotlaps/${hotlap.era_id}/charts/${encodeURIComponent(hotlap.track)}/${encodeURIComponent(hotlap.vehicle)}`,
	);
	const splits = $derived(intermediateSplits(hotlap));
	const sectors = $derived(sectorTimes(hotlap));
	const sectorComparison = $derived(
		data.worldRecord && hotlap.state === 'valid'
			? compareSectors(hotlap, data.worldRecord)
			: null,
	);
	let hoveredSector = $state<number | null>(null);
	let focusedSector = $state<number | null>(null);
	let selectedSector = $state<number | null>(null);
	const activeSector = $derived(
		hoveredSector ?? focusedSector ?? selectedSector,
	);
	let confirmOpen = $state(false);
	let busy = $state(false);
	let actionError = $state('');
	let previousLap = $state<number | null>(null);
	$effect(() => {
		if (hotlap.id !== previousLap) {
			previousLap = hotlap.id;
			actionError = '';
			confirmOpen = false;
			hoveredSector = null;
			focusedSector = null;
			selectedSector = null;
		}
	});
	async function remove() {
		if (!owner || busy) return;
		const id = hotlap.id;
		busy = true;
		actionError = '';
		try {
			await api.hotlaps.remove({
				hotlap: id,
				'X-CSRF-Token': session.me.csrf_token,
			});
			confirmOpen = false;
			await goto('/account/hotlaps', { invalidateAll: true });
		} catch (cause) {
			actionError = apiErrorMessage(cause);
		} finally {
			busy = false;
		}
	}
	async function validate() {
		if (!owner || busy) return;
		busy = true;
		actionError = '';
		try {
			await api.hotlaps.validateForTesting({
				hotlap: hotlap.id,
				'X-CSRF-Token': session.me.csrf_token,
			});
			await invalidate('app:hotlaps');
		} catch (cause) {
			actionError = apiErrorMessage(cause);
		} finally {
			busy = false;
		}
	}
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<div class="mx-auto w-full max-w-screen-2xl px-4 py-4 md:px-6 md:py-3">
		<div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
			<div class="w-full min-w-0 md:w-auto">
				<Breadcrumbs items={data.breadcrumbs} />
			</div>

			<div
				class="flex w-full min-w-0 flex-wrap items-center gap-2 border-t pt-3 md:ml-auto md:w-auto md:border-t-0 md:pt-0"
				aria-label="Hotlap actions"
			>
				{#if hotlap.spr}<Button size="sm" href={hotlap.spr.url}
						><DownloadIcon />Download SPR replay</Button
					>{/if}
				{#if hotlap.raf}<Button
						size="sm"
						variant="outline"
						href={hotlap.raf.url}><DownloadIcon />Download RAF telemetry</Button
					>{/if}
				{#if hotlap.state === 'valid'}<ReportHotlap
						hotlap={hotlap.id}
						existingReport={hotlap.reports}
						label="Report this hotlap"
					/>{/if}
				{#if owner}
					{#if session.me.allow_test_validation && hotlap.state !== 'valid'}<Button
							variant="outline"
							size="sm"
							onclick={validate}
							disabled={busy}>Validate for testing</Button
						>{/if}
					<Button
						variant="destructive"
						size="sm"
						onclick={() => (confirmOpen = true)}
						disabled={busy}><Trash2Icon />Remove my hotlap</Button
					>
				{/if}
			</div>
			{#if actionError && !confirmOpen}<p
					role="alert"
					class="text-sm text-destructive"
				>
					{actionError}
				</p>{/if}
		</div>
	</div>
	<PageIdentity label="Hotlap identity">
		<div class="space-y-3 py-2">
			<div class="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-2">
				<div class="flex min-w-0 items-center gap-3">
					<Flag
						code={hotlap.player.flag_code}
						fallback={hotlap.player.country_code}
						size="lg"
					/>
					<h1 class="racing-page-title min-w-0 break-words">
						<a
							class="hover:underline"
							href={`/drivers/${encodeURIComponent(hotlap.player.lfs_username)}`}
						>
							{hotlap.player.display_name || hotlap.player.lfs_username}
						</a>
					</h1>
				</div>
				<a
					href={chartHref}
					title={data.track?.name ?? hotlap.track}
					aria-label={`View chart for ${data.track?.name ?? hotlap.track}`}
					class="inline-flex min-w-0 items-center gap-2 rounded-sm py-1 outline-none hover:bg-muted/50 focus-visible:ring-3 focus-visible:ring-ring/50"
				>
					<Thumbnail
						src={data.track?.image?.url ?? data.track?.outline?.url ?? null}
						class="w-12 shrink-0 bg-transparent sm:w-16"
					/>
					<span class="racing-section-title break-all">{hotlap.track}</span>
				</a>
				<a
					href={chartHref}
					title={data.vehicle?.name ?? hotlap.vehicle}
					aria-label={`View chart for ${data.vehicle?.name ?? hotlap.vehicle}`}
					class="inline-flex min-w-0 items-center gap-2 rounded-sm py-1 outline-none hover:bg-muted/50 focus-visible:ring-3 focus-visible:ring-ring/50"
				>
					<Thumbnail
						src={data.vehicle?.image?.url ?? null}
						class="w-12 shrink-0 bg-transparent sm:w-16"
					/>
					<span class="racing-section-title break-all">{hotlap.vehicle}</span>
				</a>
			</div>
			<div
				class="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm text-muted-foreground"
				aria-label="Lap details"
			>
				<span>Set <DateTime value={hotlap.created_at} /></span>
				<div class="flex flex-wrap items-center gap-2">
					<Badge variant={status.variant}>{status.label}</Badge>
					<Badge variant="outline">LFS {hotlap.game_version}</Badge>
					<Badge variant="outline">{steeringLabels[hotlap.steering]}</Badge>
					{#each controlTags(hotlap) as tag (tag)}<Badge variant="outline"
							>{tag}</Badge
						>{/each}
				</div>
			</div>
		</div>
		{#snippet highlight()}
			<RecordHighlights class="lg:w-[32rem]">
				<RecordHighlight
					title="Lap time"
					variant="cyan"
					class="w-auto px-8 lg:w-auto lg:px-10"
				>
					<p class={recordHighlightValueClass}>
						{lapTime(hotlap.lap_time_ms)}
					</p>
				</RecordHighlight>
				<RecordHighlight
					title="Chart position"
					variant="purple"
					class="w-auto px-8 lg:w-auto lg:px-10"
				>
					<a
						href={chartHref}
						class={`${recordHighlightValueClass} hover:underline`}
					>
						{hotlap.position !== null
							? `#${hotlap.position.toLocaleString()}`
							: 'Unranked'}
					</a>
				</RecordHighlight>
			</RecordHighlights>
		{/snippet}
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-8 p-4 md:p-6">
		{#if splits.length}
			<section class="space-y-3" aria-labelledby="hotlap-splits-title">
				<h2
					id="hotlap-splits-title"
					class="racing-section-title flex h-8 items-center"
				>
					Split times
				</h2>
				<TableFrame appearance="data">
					<figure class="min-w-0" aria-labelledby="hotlap-splits-title">
						<div
							class={data.track?.outline
								? 'grid min-w-0 items-center lg:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)]'
								: 'min-w-0'}
						>
							{#if data.track?.outline}
								<TrackSectorMap
									outline={data.track.outline}
									track={hotlap.track}
									comparison={sectorComparison}
									bind:hoveredSector
									bind:focusedSector
									bind:selectedSector
								/>
							{/if}
							<div
								class={data.track?.outline
									? 'min-w-0 overflow-x-auto border-t lg:border-t-0 lg:border-l'
									: 'min-w-0 overflow-x-auto'}
							>
								<Table.Root
									class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4"
								>
									<Table.Caption id="hotlap-sector-legend" class="sr-only"
										>Split timing legend. Negative differences are faster than
										the current world record.</Table.Caption
									>
									<Table.Header>
										<Table.Row>
											<Table.Head>Split</Table.Head>
											<Table.Head class="text-right">Elapsed</Table.Head>
											<Table.Head class="text-right">Time</Table.Head>
											{#if sectorComparison}<Table.Head class="text-right"
													>WR</Table.Head
												><Table.Head class="text-right">Difference</Table.Head
												>{/if}
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#each [...splits, hotlap.lap_time_ms] as time, index (index)}
											<Table.Row
												data-state={activeSector === index
													? 'selected'
													: undefined}
												onpointerenter={(event) => {
													if (
														event.pointerType !== 'touch' &&
														sectorComparison &&
														data.track?.outline
													)
														hoveredSector = index;
												}}
												onpointerleave={() => {
													if (hoveredSector === index) hoveredSector = null;
												}}
											>
												<th
													scope="row"
													data-slot="table-cell"
													class="py-2 text-left font-medium"
												>
													{#if sectorComparison && data.track?.outline}
														<button
															type="button"
															class="cursor-pointer whitespace-nowrap text-left font-medium outline-none focus-visible:underline"
															aria-pressed={selectedSector === index}
															onfocus={() => (focusedSector = index)}
															onblur={() => {
																if (focusedSector === index)
																	focusedSector = null;
															}}
															onclick={() =>
																(selectedSector =
																	selectedSector === index ? null : index)}
															onkeydown={(event) => {
																if (event.key === 'Escape')
																	selectedSector = null;
															}}>Split {index + 1}</button
														>
													{:else}<span class="whitespace-nowrap"
															>Split {index + 1}</span
														>{/if}
												</th>
												<Table.Cell
													class="text-right font-mono tabular-nums text-muted-foreground"
													>{lapTime(time)}</Table.Cell
												>
												<Table.Cell
													class="text-right font-mono font-medium tabular-nums"
													style={`color: ${sectorColour(sectorComparison?.[index]?.delta)}`}
													>{lapTime(sectors[index])}</Table.Cell
												>
												{#if sectorComparison}
													<Table.Cell class="text-right font-mono tabular-nums"
														>{lapTime(
															sectorComparison[index].reference,
														)}</Table.Cell
													>
													<Table.Cell
														class="text-right font-mono tabular-nums"
														style={`color: ${sectorColour(sectorComparison[index].delta)}`}
														>{delta(sectorComparison[index].delta)}</Table.Cell
													>
												{/if}
											</Table.Row>
										{/each}
									</Table.Body>
								</Table.Root>
							</div>
						</div>
					</figure>
				</TableFrame>
			</section>
		{/if}
		{#if owner && hotlap.submission}
			<aside
				class="space-y-2 border-l-2 border-primary bg-muted/50 p-4 text-sm"
				aria-label="Your submission details"
			>
				<h2 class="font-semibold">Your submission</h2>
				<p>
					Replay vehicle: {hotlap.submission.raw_vehicle_name}{hotlap.submission
						.mod_version === null
						? ''
						: ` (mod version ${hotlap.submission.mod_version})`}
				</p>
				{#if hotlap.submission.hlvc_result_code !== null}<p>
						Validation result: {hotlap.submission.hlvc_result_code}
					</p>{/if}
				{#if hotlap.submission.error_detail}<p
						class="whitespace-pre-wrap break-words text-destructive"
					>
						{hotlap.submission.error_detail}
					</p>{/if}
			</aside>
		{/if}
		{#if data.comments}
			<div class="space-y-3">
				<h2 id="hotlap-conversation-title" class="racing-section-title">
					Conversation
				</h2>
				<HotlapComments hotlap={hotlap.id} comments={data.comments} />
			</div>
		{/if}
	</div>
</main>

<ConfirmDialog
	bind:open={confirmOpen}
	title="Remove your hotlap?"
	description={`This removes hotlap #${hotlap.id} and updates the rankings. Its conversation and reports will remain public. This cannot be undone.`}
	confirmLabel="Remove hotlap"
	{busy}
	error={actionError}
	onConfirm={remove}
/>
