<script lang="ts">
	import PlayerBadge from '$lib/components/app/PlayerBadge.svelte';
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import Empty from '$lib/components/app/Empty.svelte';
	import Breadcrumbs from '$lib/components/app/Breadcrumbs.svelte';
	import { dateTime, delta, lapTime } from '$lib/format.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const player = $derived(data.player);

	const totals = $derived([
		{ label: 'Hotlaps', value: player.stats.hotlaps },
		{ label: 'Personal bests', value: player.stats.personal_bests },
		{ label: 'World records', value: player.stats.world_records },
		{ label: 'Podiums', value: player.stats.podiums },
		{ label: 'Eras', value: player.stats.eras },
	]);
</script>

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<div class="flex flex-wrap items-center justify-between gap-4 text-sm">
		<Breadcrumbs items={data.breadcrumbs}>
			{#snippet trailing()}
				<CompareButton driver={player} />
			{/snippet}
		</Breadcrumbs>
		{#if player.stats.first_hotlap_at}
			<p class="ml-auto text-muted-foreground">
				Racing here since {dateTime(player.stats.first_hotlap_at)}
			</p>
		{/if}
	</div>
	<dl
		class="grid grid-cols-2 border-y py-3 text-sm sm:flex sm:flex-wrap sm:items-center sm:gap-y-3"
	>
		{#each totals as total (total.label)}
			<div
				class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 px-4 py-1 odd:border-r last:border-r-0 sm:border-r sm:px-6 sm:py-0 sm:first:pl-0 sm:last:border-r-0"
			>
				<dt class="text-muted-foreground">{total.label}</dt>
				<dd class="font-semibold tabular-nums">
					{total.value.toLocaleString()}
				</dd>
			</div>
		{/each}
	</dl>

	{#if player.eras.length}
		<Card.Root class="gap-0 py-0">
			<Card.Header class="pt-4">
				<Card.Title>
					<h2>Record by era</h2>
				</Card.Title>
			</Card.Header>
			<Card.Content
				class="px-0 [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
			>
				<Table.Root>
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
			</Card.Content>
		</Card.Root>
	{/if}

	<Card.Root class="gap-0 py-0">
		<Card.Header class="pt-4">
			<Card.Title>
				<h2>Highlights</h2>
			</Card.Title>
			<Card.Description
				>Current personal bests with the strongest chart positions.</Card.Description
			>
		</Card.Header>
		<Card.Content
			class="px-0 [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
		>
			{#if player.highlights.length}
				<Table.Root>
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
						{#each player.highlights as result (result.hotlap_id)}
							<Table.Row>
								<Table.Cell>
									<a
										class="hover:underline"
										href="/hotlaps/{result.era_id}/charts/{result.track}/{result.vehicle}"
									>
										{result.track} · {result.vehicle}
									</a>
								</Table.Cell>
								<Table.Cell class="text-muted-foreground"
									>{result.era_id}</Table.Cell
								>
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
									#{result.position} of {result.entries.toLocaleString()}
								</Table.Cell>
								<Table.Cell class="text-muted-foreground"
									>{dateTime(result.created_at)}</Table.Cell
								>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			{:else}
				<Empty title="No published laps yet">
					<p>{player.display_name} has no validated hotlaps.</p>
				</Empty>
			{/if}
		</Card.Content>
	</Card.Root>
</main>
