<script lang="ts">
	import { onMount } from 'svelte';
	import { invalidate } from '$app/navigation';
	import { lapTime, delta } from '#lib/format.js';
	import DateTime from '#lib/components/app/DateTime.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let now = $state(Date.now());
	const status = $derived(
		now < Date.parse(data.event.starts_at)
			? 'Upcoming'
			: data.event.ends_at && now >= Date.parse(data.event.ends_at)
				? 'Completed'
				: 'Live',
	);
	const mode = $derived(data.event.game?.source.type);
	const modeName = $derived(
		mode === 'bomb'
			? 'Bomb Run'
			: mode === 'metronome'
				? 'Metronome'
				: mode === 'shortcut'
					? 'Shortcut'
					: null,
	);
	let refreshFailed = $state(false);
	onMount(() => {
		let refreshing = false;
		let finalizedEvent: number | null = null;
		const timer = window.setInterval(async () => {
			now = Date.now();
			if (
				refreshing ||
				(status === 'Completed' && finalizedEvent === data.event.id)
			)
				return;
			refreshing = true;
			try {
				await invalidate('app:event');
				if (status === 'Completed') finalizedEvent = data.event.id;
				refreshFailed = false;
			} catch {
				refreshFailed = true;
			} finally {
				refreshing = false;
			}
		}, 10000);
		return () => window.clearInterval(timer);
	});
</script>

<svelte:head><title>{data.event.title} | lfspla.net</title></svelte:head>
<main
	id="main"
	class="mx-auto w-full max-w-screen-xl flex-1 space-y-6 p-4 md:p-6"
>
	<PageHeading title={data.event.title}
		>{modeName ?? 'Event'} · {status}</PageHeading
	>
	<dl class="flex flex-wrap gap-6 text-sm">
		<div>
			<dt class="text-muted-foreground">Starts</dt>
			<dd><DateTime value={data.event.starts_at} /></dd>
		</div>
		{#if data.event.ends_at}<div>
				<dt class="text-muted-foreground">Ends</dt>
				<dd><DateTime value={data.event.ends_at} /></dd>
			</div>{/if}
		{#if data.event.game}<div>
				<dt class="text-muted-foreground">Track</dt>
				<dd>{data.event.game.track}</dd>
			</div>{/if}
	</dl>
	{#if data.event.description}<p class="whitespace-pre-line">
			{data.event.description}
		</p>{/if}
	{#if data.event.game}
		<section class="space-y-3" aria-labelledby="rules-heading">
			<h2 id="rules-heading" class="text-xl font-semibold">How to play</h2>
			{#if data.event.game.source.type === 'bomb'}
				<p>
					Reach checkpoints to keep the timer alive. Most checkpoints wins, then
					longest survival time.
				</p>
				<p class="text-sm text-muted-foreground">
					Checkpoint timer: {data.event.game.source.checkpoint_timeout_secs}s.
					Penalty per checkpoint: {data.event.game.source
						.checkpoint_penalty_ms}ms. Maximum collision penalty: {data.event
						.game.source.collision_max_penalty_ms}ms.
				</p>
			{:else if data.event.game.source.type === 'metronome'}
				<p>
					Match the target lap time of {lapTime(
						data.event.game.source.target_ms,
					)}. The smallest absolute difference wins.
				</p>
			{:else}<p>
					Find the fastest route through the layout. Your fastest completed
					attempt counts.
				</p>{/if}
			<p>
				Vehicles: {data.event.game.vehicles?.length
					? data.event.game.vehicles
							.map(
								(rule) =>
									`${rule.vehicle}${rule.added_mass_kg ? ` +${rule.added_mass_kg}kg` : ''}${rule.intake_restriction ? ` / ${rule.intake_restriction}% intake restriction` : ''}`,
							)
							.join(', ')
					: 'Unrestricted'}
			</p>
			<div class="flex flex-wrap gap-3">
				{#if status === 'Live' && data.event.join_url}<Button
						href={data.event.join_url}>Join server</Button
					>{/if}
				{#if data.event.layout_url}<Button
						variant="outline"
						href={data.event.layout_url}>Download layout</Button
					>{/if}
			</div>
		</section>
		<section class="space-y-3" aria-labelledby="results-heading">
			<h2 id="results-heading" class="text-xl font-semibold">
				{status === 'Completed' ? 'Results' : 'Standings'}
			</h2>
			{#if refreshFailed}<p role="status" class="text-muted-foreground">
					Results could not be refreshed. Trying again shortly.
				</p>{/if}
			{#if data.results.items.length}
				<div class="overflow-x-auto">
					<table class="w-full text-left text-sm">
						<thead
							><tr class="border-b"
								><th class="p-3">Position</th><th class="p-3">Driver</th
								>{#if mode !== 'metronome'}<th class="p-3">Vehicle</th>{/if}<th
									class="p-3"
									>{mode === 'bomb'
										? 'Checkpoints'
										: mode === 'metronome'
											? 'Difference'
											: 'Time'}</th
								>{#if mode === 'bomb'}<th class="p-3">Survival</th>{/if}</tr
							></thead
						>
						<tbody
							>{#each data.results.items as row (row.player_id)}<tr
									class="border-b"
									><td class="p-3 tabular-nums">{row.position}</td><td
										class="p-3"
										><a
											class="underline"
											href={`/drivers/${encodeURIComponent(row.username)}`}
											>{row.display_name}</a
										></td
									>{#if mode !== 'metronome'}<td class="p-3">{row.vehicle}</td
										>{/if}<td class="p-3 tabular-nums"
										>{mode === 'bomb'
											? row.checkpoint_count
											: mode === 'metronome'
												? delta(row.delta_ms)
												: lapTime(row.time_ms)}</td
									>{#if mode === 'bomb'}<td class="p-3 tabular-nums"
											>{lapTime(row.survival_ms)}</td
										>{/if}</tr
								>{/each}</tbody
						>
					</table>
				</div>
			{:else}<p class="text-muted-foreground">
					{status === 'Upcoming'
						? 'Standings will appear when the event starts.'
						: 'No results recorded yet.'}
				</p>{/if}
			<nav aria-label="Results pages" class="flex gap-3">
				{#if data.results.pagination.page > 1}<Button
						variant="outline"
						href={`?page=${data.results.pagination.page - 1}`}>Previous</Button
					>{/if}
				{#if data.results.pagination.page < data.results.pagination.total_pages}<Button
						variant="outline"
						href={`?page=${data.results.pagination.page + 1}`}>Next</Button
					>{/if}
			</nav>
		</section>
	{/if}
</main>
