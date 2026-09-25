<script lang="ts">
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import PageHeading from '$lib/components/app/PageHeading.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import * as Table from '$lib/components/ui/table/index.js';
	import { dateTime, lapTime } from '$lib/format.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { hotlapPath } from '$lib/era.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const stats = $derived([
		{ label: 'Hotlaps', value: data.stats?.validated_hotlaps },
		{ label: 'Drivers', value: data.stats?.drivers },
		{ label: 'Combinations', value: data.stats?.combinations },
		{ label: 'Eras', value: data.stats?.eras },
	]);
</script>

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<section class="flex flex-wrap items-center justify-between gap-6">
		<PageHeading title="A new home for Live for Speed data.">
			A community-built alternative to LFS World-starting with validated hotlaps
			and (hopefully) growing to include online results, personal bests,
			statistics, and driver history.
		</PageHeading>
	</section>
	<dl
		class="grid grid-cols-2 border-y py-3 text-sm sm:flex sm:flex-wrap sm:items-center sm:gap-y-3"
	>
		{#each stats as stat (stat.label)}
			<div
				class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 px-4 py-1 odd:border-r last:border-r-0 sm:border-r sm:px-6 sm:py-0 sm:first:pl-0"
			>
				<dt class="text-muted-foreground">{stat.label}</dt>
				<dd class="font-semibold tabular-nums">
					{stat.value?.toLocaleString() ?? '-'}
				</dd>
			</div>
		{/each}
	</dl>

	<div class="min-w-0">
		<Card.Root class="min-w-0">
			<Card.Header>
				<Card.Title><h2>Latest validated hotlaps</h2></Card.Title>
				<Card.Description
					>Positions show current chart standings; - means the upload is not a
					ranked personal best.</Card.Description
				>
			</Card.Header>
			<Table.Root>
				<Table.Header
					><Table.Row>
						{#each ['Submitted', 'Driver', 'Era', 'Track / Vehicle', '#', 'Lap time'] as heading}
							<Table.Head class="first:pl-4 last:pr-4">{heading}</Table.Head>
						{/each}
					</Table.Row></Table.Header
				>
				<Table.Body>
					{#each data.activity as lap (lap.id)}
						<Table.Row>
							<Table.Cell class="pl-4 text-muted-foreground"
								><time datetime={lap.created_at}
									>{dateTime(lap.created_at)}</time
								></Table.Cell
							>
							<Table.Cell class="font-medium">
								<div class="flex min-w-0 items-center gap-1">
									<Flag code={lap.player.country_code} />
									<a
										class="min-w-0 truncate hover:underline"
										href="/drivers/{encodeURIComponent(
											lap.player.lfs_username,
										)}">{lap.player.display_name}</a
									>
									<CompareButton driver={lap.player} />
								</div>
							</Table.Cell>
							<Table.Cell
								><a class="hover:underline" href={hotlapPath(lap.era_id)}
									><Badge variant="secondary"
										>{data.eras.find((era) => era.id === lap.era_id)?.title ??
											lap.era_id}</Badge
									></a
								></Table.Cell
							>
							<Table.Cell
								>{#if lap.vehicle}<a
										class="hover:underline"
										href={`${hotlapPath(lap.era_id)}/charts/${encodeURIComponent(lap.track)}/${encodeURIComponent(lap.vehicle)}`}
										>{lap.track} / {lap.vehicle}</a
									>{:else}{lap.track} / -{/if}</Table.Cell
							>
							<Table.Cell class="tabular-nums">{lap.position ?? '-'}</Table.Cell
							>
							<Table.Cell class="pr-4 font-mono tabular-nums"
								>{lapTime(lap.lap_time_ms)}</Table.Cell
							>
						</Table.Row>
					{:else}
						<Table.Row
							><Table.Cell
								colspan={7}
								class="px-4 py-8 text-center text-muted-foreground"
								>{data.activityUnavailable
									? 'Hotlap activity is currently unavailable.'
									: 'No validated hotlaps yet.'}</Table.Cell
							></Table.Row
						>
					{/each}
				</Table.Body>
			</Table.Root>
		</Card.Root>
	</div>
</main>
