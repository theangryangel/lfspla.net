<script lang="ts">
	import { onMount } from 'svelte';
	import { invalidate } from '$app/navigation';
	import { lapTime, delta } from '#lib/format.js';
	import Breadcrumbs from '#lib/components/app/Breadcrumbs.svelte';
	import PaginationControls from '#lib/components/app/PaginationControls.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import Empty from '#lib/components/app/Empty.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import DateTime from '#lib/components/app/DateTime.svelte';
	import EventIdentity from '#lib/components/app/EventIdentity.svelte';
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
	const breadcrumbs = $derived(
		data.breadcrumbs.map((crumb, index) =>
			index === data.breadcrumbs.length - 1
				? { ...crumb, tag: { label: status, accent: status === 'Live' } }
				: crumb,
		),
	);
	const mode = $derived(data.event.game?.source.type);
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
<main id="main" class="w-full min-w-0 flex-1">
	<div class="mx-auto w-full max-w-screen-2xl px-4 py-4 md:px-6 md:py-3">
		<Breadcrumbs items={breadcrumbs} />
	</div>
	<EventIdentity event={data.event} {status} leader={data.leader} />
	<div class="mx-auto w-full max-w-screen-2xl space-y-6 p-4 md:p-6">
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
			{#if data.event.game}
				<div>
					<dt class="text-muted-foreground">Vehicles</dt>
					<dd>
						{data.event.game.vehicles?.length
							? data.event.game.vehicles
									.map(
										(rule) =>
											`${rule.vehicle}${rule.added_mass_kg ? ` +${rule.added_mass_kg}kg` : ''}${rule.intake_restriction ? ` / ${rule.intake_restriction}% intake restriction` : ''}`,
									)
									.join(', ')
							: 'Unrestricted'}
					</dd>
				</div>
			{/if}
		</dl>
		{#if data.event.description}<p class="whitespace-pre-line">
				{data.event.description}
			</p>{/if}
		{#if data.event.game}
			<section class="space-y-3" aria-labelledby="results-heading">
				<h2 id="results-heading" class="racing-section-title">
					{status === 'Completed' ? 'Results' : 'Standings'}
				</h2>
				{#if refreshFailed}<p role="status" class="text-muted-foreground">
						Results could not be refreshed. Trying again shortly.
					</p>{/if}
				{#if data.results.items.length}
					<TableFrame appearance="data">
						<Table.Root
							aria-label={status === 'Completed'
								? 'Event results'
								: 'Event standings'}
						>
							<Table.Header>
								<Table.Row>
									<Table.Head class="text-center">#</Table.Head>
									<Table.Head>Driver</Table.Head>
									{#if mode !== 'metronome'}<Table.Head>Vehicle</Table.Head
										>{/if}
									<Table.Head class="text-right"
										>{mode === 'bomb'
											? 'Checkpoints'
											: mode === 'metronome'
												? 'Difference'
												: 'Time'}</Table.Head
									>
									{#if mode === 'bomb'}<Table.Head class="text-right"
											>Survival</Table.Head
										>{/if}
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each data.results.items as row (row.player_id)}
									<Table.Row>
										<Table.Cell
											class={row.position === 1
												? 'text-center tabular-nums text-time-best shadow-[inset_3px_0_0_var(--time-best)]'
												: 'text-center tabular-nums'}>{row.position}</Table.Cell
										>
										<Table.Cell>
											<div class="flex w-max items-center gap-1">
												<Flag
													code={row.flag_code}
													fallback={row.country_code}
												/>
												<a
													class="shrink-0 whitespace-nowrap font-medium hover:underline"
													href={`/drivers/${encodeURIComponent(row.username)}`}
													>{row.display_name}</a
												>
											</div>
										</Table.Cell>
										{#if mode !== 'metronome'}<Table.Cell
												>{row.vehicle}</Table.Cell
											>{/if}
										<Table.Cell class="text-right font-mono tabular-nums"
											>{mode === 'bomb'
												? row.checkpoint_count
												: mode === 'metronome'
													? delta(row.delta_ms)
													: lapTime(row.time_ms)}</Table.Cell
										>
										{#if mode === 'bomb'}<Table.Cell
												class="text-right font-mono tabular-nums"
												>{lapTime(row.survival_ms)}</Table.Cell
											>{/if}
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					</TableFrame>
				{:else}
					<Empty
						title={status === 'Upcoming'
							? 'Standings will appear when the event starts.'
							: 'No results recorded yet.'}
					/>
				{/if}
				<PaginationControls
					pagination={data.results.pagination}
					label="drivers"
				/>
			</section>
		{/if}
	</div>
</main>
