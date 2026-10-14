<script lang="ts">
	import PlayerBadge from '$lib/components/app/PlayerBadge.svelte';
	import CompareButton from '$lib/components/app/CompareButton.svelte';
	import TableFrame from '$lib/components/app/TableFrame.svelte';
	import PageIdentity from '$lib/components/app/PageIdentity.svelte';
	import RecordHighlight from '$lib/components/app/RecordHighlight.svelte';
	import Flag from '$lib/components/app/Flag.svelte';
	import * as Table from '$lib/components/ui/table/index.js';
	import HotlapActivity from '$lib/components/app/HotlapActivity.svelte';
	import Breadcrumbs from '$lib/components/app/Breadcrumbs.svelte';
	import { dateTime } from '$lib/format.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const player = $derived(data.player);
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<div class="mx-auto w-full max-w-screen-2xl px-4 py-4 md:px-6 md:py-3">
		<Breadcrumbs items={data.breadcrumbs} />
	</div>
	<PageIdentity label="Driver identity">
		<div class="space-y-3 py-2">
			<div class="flex min-w-0 items-center gap-3">
				<Flag code={player.flag_code} fallback={player.country_code} size="lg" />
				<h1 class="racing-page-title min-w-0 flex-1 break-words">
					{player.display_name}
				</h1>
				<div class="shrink-0"><CompareButton driver={player} /></div>
			</div>
			<div
				class="flex min-w-0 flex-wrap gap-x-6 gap-y-1 text-sm text-muted-foreground"
			>
				<p class="break-words">{player.lfs_username}</p>
				{#if player.stats.first_hotlap_at}
					<p>Racing here since {dateTime(player.stats.first_hotlap_at)}</p>
				{/if}
			</div>
		</div>
		{#snippet highlight()}
			<RecordHighlight title="World records" variant="purple">
				<p class="text-right text-3xl font-bold tabular-nums">
					{player.stats.world_records.toLocaleString()}
				</p>
			</RecordHighlight>
		{/snippet}
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-6 p-4 md:p-6">
		{#if player.eras.length}
			<section class="space-y-3">
				<h2 class="racing-section-title">Record by era</h2>
				<TableFrame appearance="data">
					<Table.Root
						class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
					>
						<Table.Header>
							<Table.Row>
								<Table.Head>Era</Table.Head>
								<Table.Head class="text-right">Hotlaps</Table.Head>
								<Table.Head class="text-right">Personal bests</Table.Head>
								<Table.Head class="text-right">1st</Table.Head>
								<Table.Head class="text-right">2nd</Table.Head>
								<Table.Head class="text-right">3rd</Table.Head>
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
									<Table.Cell class="text-right tabular-nums"
										>{era.hotlaps.toLocaleString()}</Table.Cell
									>
									<Table.Cell class="text-right tabular-nums"
										>{era.personal_bests.toLocaleString()}</Table.Cell
									>
									<Table.Cell class="text-right tabular-nums"
										>{era.firsts.toLocaleString()}</Table.Cell
									>
									<Table.Cell class="text-right tabular-nums"
										>{era.seconds.toLocaleString()}</Table.Cell
									>
									<Table.Cell class="text-right tabular-nums"
										>{era.thirds.toLocaleString()}</Table.Cell
									>
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

		<HotlapActivity
			title="Hotlap activity"
			lfsUsername={player.lfs_username}
			eras={data.eras}
			refreshKey={player}
			views={['all', 'world_records']}
			appearance="data"
		/>
	</div>
</main>
