<script lang="ts">
	import type { PageProps } from './$types';
	import { useApi, apiErrorMessage } from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import { goto, invalidate } from '$app/navigation';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import RecordHighlight from '#lib/components/app/RecordHighlight.svelte';
	import Breadcrumbs from '#lib/components/app/Breadcrumbs.svelte';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { page } from '$app/state';
	import DateTime from '#lib/components/app/DateTime.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
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
	const selectedTab = $derived(
		data.comments &&
			(page.url.searchParams.get('tab') === 'conversation' ||
				page.url.searchParams.has('around') ||
				page.url.searchParams.has('comment') ||
				page.url.hash.startsWith('#comment'))
			? 'conversation'
			: 'overview',
	);
	function selectTab(value: string) {
		if (value === selectedTab) return;
		const url = new URL(page.url.href);
		url.searchParams.set('tab', value);
		if (value === 'overview') {
			url.searchParams.delete('around');
			url.searchParams.delete('comment');
			if (url.hash.startsWith('#comment')) url.hash = '';
		}
		void goto(url, { replaceState: true, reset: false });
	}
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
		<div class="space-y-4 py-2">
			<div class="flex flex-wrap items-center gap-3">
				<h1 class="racing-page-title">
					Hotlap #{hotlap.id} by
					<a
						class="hover:underline"
						href={`/drivers/${encodeURIComponent(hotlap.player.lfs_username)}`}
					>
						<Flag
							code={hotlap.player.flag_code}
							fallback={hotlap.player.country_code}
						/>
						{hotlap.player.display_name || hotlap.player.lfs_username}
					</a>
					on
					<a class="hover:underline" href={chartHref}
						>{hotlap.era_title} / {hotlap.track} / {hotlap.vehicle}</a
					>
				</h1>
				<Badge variant={status.variant}>{status.label}</Badge>
			</div>
		</div>
		{#snippet highlight()}
			<RecordHighlight title="Lap time" variant="cyan">
				<p
					class="text-right font-mono text-4xl font-bold tracking-tight tabular-nums"
				>
					{lapTime(hotlap.lap_time_ms)}
				</p>
			</RecordHighlight>
		{/snippet}
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-8 p-4 md:p-6">
		<Tabs.Root value={selectedTab} onValueChange={selectTab} class="space-y-5">
			<Tabs.List aria-label="Hotlap sections">
				<Tabs.Trigger value="overview">Overview</Tabs.Trigger>
				{#if data.comments}<Tabs.Trigger value="conversation"
						>Conversation</Tabs.Trigger
					>{/if}
			</Tabs.List>
			<Tabs.Content value="overview" class="space-y-4">
				<dl
					class="grid gap-5 border-y bg-card px-4 py-5 text-sm sm:grid-cols-2 lg:grid-cols-4"
				>
					<div>
						<dt class="text-muted-foreground">Set</dt>
						<dd class="mt-1"><DateTime value={hotlap.created_at} /></dd>
					</div>
					<div>
						<dt class="text-muted-foreground">LFS version</dt>
						<dd class="mt-1">{hotlap.game_version}</dd>
					</div>
					<div>
						<dt class="text-muted-foreground">Controller</dt>
						<dd class="mt-1">{steeringLabels[hotlap.steering]}</dd>
					</div>
					<div>
						<dt class="text-muted-foreground">Controls</dt>
						<dd class="mt-2 flex flex-wrap gap-2">
							{#each controlTags(hotlap) as tag (tag)}<Badge variant="outline"
									>{tag}</Badge
								>{/each}
						</dd>
					</div>
				</dl>
				{#if splits.length}
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
				{/if}
				{#if owner && hotlap.submission}
					<aside
						class="space-y-2 border-l-2 border-primary bg-muted/50 p-4 text-sm"
						aria-label="Your submission details"
					>
						<h3 class="font-semibold">Your submission</h3>
						<p>
							Replay vehicle: {hotlap.submission.raw_vehicle_name}{hotlap
								.submission.mod_version === null
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
			</Tabs.Content>
			{#if data.comments}
				<Tabs.Content value="conversation">
					<HotlapComments hotlap={hotlap.id} comments={data.comments} />
				</Tabs.Content>
			{/if}
		</Tabs.Root>
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
