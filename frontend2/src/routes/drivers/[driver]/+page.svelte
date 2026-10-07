<script lang="ts">
	import PlayerBadge from '$lib/components/app/PlayerBadge.svelte';
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import TableFrame from '$lib/components/app/TableFrame.svelte';
	import StatsBar from '$lib/components/app/StatsBar.svelte';
	import * as Table from '$lib/components/ui/table/index.js';
	import Empty from '$lib/components/app/Empty.svelte';
	import Breadcrumbs from '$lib/components/app/Breadcrumbs.svelte';
	import { dateTime, delta, lapTime } from '$lib/format.js';
	import type { PageProps } from './$types';
	import {Badge} from "$lib/components/ui/badge";

	let { data }: PageProps = $props();

	const player = $derived(data.player);

	const totals = $derived([
		{ label: 'Hotlaps', value: player.stats.hotlaps },
		{ label: 'Personal bests', value: player.stats.personal_bests },
		{ label: 'World records', value: player.stats.world_records },
		{ label: 'Podiums', value: player.stats.podiums },
		{ label: 'Eras', value: player.stats.eras },
		...(player.stats.first_hotlap_at
			? [
					{
						label: 'Racing here since',
						value: dateTime(player.stats.first_hotlap_at),
						valueFirst: false,
					},
				]
			: []),
	]);
</script>

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<div class="flex flex-wrap items-center gap-4 text-sm">
		<Breadcrumbs items={data.breadcrumbs}>
			{#snippet trailing()}
				<CompareButton driver={player} />
			{/snippet}
		</Breadcrumbs>
		<StatsBar items={totals} class="ml-auto justify-end" />
	</div>

	{#if player.eras.length}
		<section class="space-y-3">
			<h2 class="text-lg font-semibold">Record by era</h2>
			<TableFrame>
				<Table.Root
					class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
				>
					<Table.Header>
						<Table.Row>
							<Table.Head>Era</Table.Head>
							<Table.Head>Hotlaps</Table.Head>
							<Table.Head>Personal bests</Table.Head>
							<Table.Head>Podiums</Table.Head>
							<Table.Head>Badges</Table.Head>
							<Table.Head>Last lap</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each player.eras as era (era.id)}
							<Table.Row>
								<Table.Cell class="font-medium">
									<a class="hover:underline" href="/hotlaps/{era.id}/rankings"
										>{era.title}</a
									>
								</Table.Cell>
								<Table.Cell class="tabular-nums"
									>{era.hotlaps.toLocaleString()}</Table.Cell
								>
								<Table.Cell class="tabular-nums"
									>{era.personal_bests.toLocaleString()}</Table.Cell
								>
								<Table.Cell class="tabular-nums">
									{era.firsts} · {era.seconds} · {era.thirds}
								</Table.Cell>
								<Table.Cell>
									{#each era.badges as playerBadge, i (i)}
										<PlayerBadge badge={playerBadge} class="mr-2" />
									{:else}
										<span class="text-muted-foreground">-</span>
									{/each}
								</Table.Cell>
								<Table.Cell class="text-muted-foreground">
									{dateTime(era.latest_hotlap_at)}
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</TableFrame>
		</section>
	{/if}

	<section class="space-y-3">
		<header class="space-y-1">
			<h2 class="text-lg font-semibold">Highlights</h2>
			<p class="text-sm text-muted-foreground">
				Current personal bests with the strongest chart positions.
			</p>
		</header>
		{#if player.highlights.length}
			<TableFrame>
				<Table.Root
					class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
				>
					<Table.Header>
						<Table.Row>
							<Table.Head>Chart</Table.Head>
							<Table.Head>Era</Table.Head>
							<Table.Head>Lap time</Table.Head>
							<Table.Head title="Difference from the current world record"
								>To WR</Table.Head
							>
							<Table.Head>#</Table.Head>
							<Table.Head>Set</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each player.highlights as result (result.id)}
							<Table.Row>
								<Table.Cell>
									<a
										class="hover:underline"
										href="/hotlaps/{result.era_id}/charts/{result.track}/{result.vehicle}"
									>
										{result.track} · {result.vehicle}
									</a>
								</Table.Cell>
								<Table.Cell class="text-muted-foreground">
									<Badge variant="secondary">
										{data.eras.find((era) => era.id === result.era_id)?.title ?? result.era_id}
									</Badge>
								</Table.Cell>
								<Table.Cell class="font-mono tabular-nums">
									{lapTime(result.lap_time_ms)}
								</Table.Cell>
								<Table.Cell
									class="font-mono tabular-nums text-muted-foreground"
								>
									{delta(result.distance_to_world_record_ms)}
								</Table.Cell>
								<Table.Cell
									class="tabular-nums {result.position === 1
										? 'text-time-best'
										: ''}"
								>
									#{result.position}
								</Table.Cell>
								<Table.Cell class="text-muted-foreground"
									>{dateTime(result.created_at)}</Table.Cell
								>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</TableFrame>
		{:else}
			<Empty title="No published laps yet">
				<p>{player.display_name} has no validated hotlaps.</p>
			</Empty>
		{/if}
	</section>
</main>
