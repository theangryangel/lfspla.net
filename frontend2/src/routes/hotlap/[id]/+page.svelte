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
	import ReportHotlap from '#lib/components/app/ReportHotlap.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import ConfirmDialog from '#lib/components/app/ConfirmDialog.svelte';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import HotlapComments from '#lib/components/app/HotlapComments.svelte';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import {
		controlTags,
		hotlapStates,
		lapTime,
		steeringLabels,
	} from '#lib/format.js';
	import { intermediateSplits, sectorTimes } from '#lib/hotlap-comparison.js';
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
	let confirmOpen = $state(false);
	let busy = $state(false);
	let actionError = $state('');
	let previousLap = $state<number | null>(null);
	$effect(() => {
		if (hotlap.id !== previousLap) {
			previousLap = hotlap.id;
			actionError = '';
			confirmOpen = false;
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

<svelte:head
	><title
		>{lapTime(hotlap.lap_time_ms)} · {hotlap.track} / {hotlap.vehicle} · Hotlap #{hotlap.id}
		| lfspla.net</title
	></svelte:head
>

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
						existingReport={data.report}
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
				<h2 id="hotlap-splits-title" class="racing-section-title">
					Split times
				</h2>
				<div class="racing-data overflow-x-auto border-y">
					<table class="w-full text-sm">
						<caption class="sr-only">Split and sector times</caption>
						<thead class="bg-muted/50 text-left"
							><tr
								><th scope="col" class="p-3">Checkpoint</th><th
									scope="col"
									class="p-3">Cumulative time</th
								><th scope="col" class="p-3">Sector time</th></tr
							></thead
						>
						<tbody
							>{#each [...splits, hotlap.lap_time_ms] as time, index (index)}<tr
									class="border-t"
									><th scope="row" class="p-3 text-left font-normal"
										>{index === splits.length
											? 'Finish'
											: `Split ${index + 1}`}</th
									><td class="p-3 font-mono tabular-nums">{lapTime(time)}</td
									><td class="p-3 font-mono tabular-nums"
										>{lapTime(sectors[index])}</td
									></tr
								>{/each}</tbody
						>
					</table>
				</div>
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
