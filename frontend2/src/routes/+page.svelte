<script lang="ts">
	import Thumbnail from '$lib/components/app/Thumbnail.svelte';
	import { delta, lapTime } from '$lib/format.js';
	import HotlapActivity from '$lib/components/app/HotlapActivity.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import PageHeading from '$lib/components/app/PageHeading.svelte';
	import StatsBar from '$lib/components/app/StatsBar.svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const stats = $derived([
		{ label: 'Hotlaps', value: data.stats?.validated_hotlaps },
		{ label: 'Drivers', value: data.stats?.drivers },
		{ label: 'Combinations', value: data.stats?.combinations },
		{ label: 'Eras', value: data.stats?.eras },
	]);
	const combo = $derived(data.stats?.combo_spotlight);
	const driver = $derived(data.stats?.driver_spotlight);
</script>

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<section class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_auto] xl:items-center">
		<PageHeading title="A new home for Live for Speed data.">
			A community-built alternative to LFS World-starting with validated hotlaps
			and (hopefully) growing to include online results, personal bests,
			statistics, and driver history.
		</PageHeading>
		<StatsBar items={stats} class="xl:justify-end" />
	</section>

	<div
		class="grid min-w-0 items-start gap-8 xl:grid-cols-[minmax(0,2fr)_minmax(18rem,1fr)]"
	>
		<HotlapActivity
			refreshKey={data.stats}
			views={['all', 'world_records', 'mine']}
			perPage={10}
			eras={data.eras}
		/>
		<div class="space-y-4">
			<Card.Root class="gap-2 bg-muted/40 py-4 shadow-none ring-0">
				<Card.Header>
					<Card.Title id="home-spotlight-title" class="text-base font-semibold"
						>Combo spotlight</Card.Title
					>
					<Card.Description>
						{#if combo}
							<a
								class="hover:underline"
								href={`/hotlaps/${encodeURIComponent(combo.chart.era_id)}/charts/${encodeURIComponent(combo.chart.track.code)}/${encodeURIComponent(combo.chart.vehicle.code)}`}
							>
								{combo.chart.era_title} / {combo.chart.track.code} / {combo
									.chart.vehicle.code}
							</a>
						{:else}
							Most popular among recent uploads
						{/if}
					</Card.Description>
					{#if combo}
						<Card.Action class="flex gap-1 self-center" aria-hidden="true">
							<Thumbnail
								kind="track"
								code={combo.chart.track.code}
								class="w-12 rounded-md"
							/>
							<Thumbnail
								kind="vehicle"
								code={combo.chart.vehicle.code}
								src={combo.chart.vehicle.image_url}
								class="w-12 rounded-md"
							/>
						</Card.Action>
					{/if}
				</Card.Header>
				<Card.Content>
					{#if combo}
						<div class="space-y-1.5">
							{#each combo.leaders as entry, index (entry.player.id)}
								<div
									class="flex items-center justify-between gap-3"
									class:opacity-40={index === 2}
								>
									<div class="flex min-w-0 items-center gap-2">
										<Flag
											code={entry.player.flag_code}
											fallback={entry.player.country_code}
										/>
										<a
											class="truncate text-sm hover:underline"
											class:font-medium={entry.position === 1}
											href={`/drivers/${encodeURIComponent(entry.player.lfs_username)}`}
											>{entry.player.display_name}</a
										>
									</div>
									<p
										class="shrink-0 font-mono tabular-nums"
										class:text-sm={entry.position === 1}
										class:text-xs={entry.position !== 1}
										class:font-semibold={entry.position === 1}
										class:text-time-best={entry.position === 1}
										class:text-muted-foreground={entry.position !== 1}
									>
										{entry.position === 1
											? lapTime(entry.lap_time_ms)
											: delta(entry.distance_to_world_record_ms)}
									</p>
								</div>
							{/each}
						</div>
					{:else}
						<p class="text-sm text-muted-foreground">
							{data.stats
								? 'No validated uploads yet.'
								: 'Spotlight unavailable.'}
						</p>
					{/if}
				</Card.Content>
			</Card.Root>
			<Card.Root class="gap-2 bg-muted/40 py-4 shadow-none ring-0">
				<Card.Header>
					<Card.Title class="text-base font-semibold"
						>Driver spotlight</Card.Title
					>
					<Card.Description>Most new personal bests recently</Card.Description>
				</Card.Header>
				<Card.Content>
					{#if driver}
						<div class="flex items-center justify-between gap-3">
							<div class="flex min-w-0 items-center gap-2">
								<Flag
									code={driver.player.flag_code}
									fallback={driver.player.country_code}
								/>
								<a
									class="truncate text-sm font-medium hover:underline"
									href={`/drivers/${encodeURIComponent(driver.player.lfs_username)}`}
									>{driver.player.display_name}</a
								>
							</div>
							<p class="shrink-0 text-sm text-muted-foreground">
								{driver.recent_personal_bests}
								{driver.recent_personal_bests === 1 ? 'PB' : 'PBs'}
							</p>
						</div>
					{:else}
						<p class="text-sm text-muted-foreground">
							{data.stats
								? 'No current personal bests among recent uploads.'
								: 'Spotlight unavailable.'}
						</p>
					{/if}
				</Card.Content>
			</Card.Root>
		</div>
	</div>
</main>
