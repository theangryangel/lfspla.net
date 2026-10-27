<script lang="ts">
	import Empty from '#lib/components/app/Empty.svelte';
	import DateTime from '#lib/components/app/DateTime.svelte';
	import PlayerBadge from '#lib/components/app/PlayerBadge.svelte';
	import CompareButton from '#lib/components/app/CompareButton.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import RecordHighlight from '#lib/components/app/RecordHighlight.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import HotlapActivity from '#lib/components/app/HotlapActivity.svelte';
	import Breadcrumbs from '#lib/components/app/Breadcrumbs.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import BrandIcon from '#lib/components/app/BrandIcon.svelte';
	import DriverMedia from '#lib/components/app/DriverMedia.svelte';
	import { useSession } from '#lib/session.svelte.js';
	import Play from '@lucide/svelte/icons/play';
	import Pencil from '@lucide/svelte/icons/pencil';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const player = $derived(data.player);
	const session = useSession();
	const ownProfile = $derived(
		session.player?.lfs_username.toLowerCase() ===
			player.lfs_username.toLowerCase(),
	);
	const links = $derived(player.profile_links);
	let media = $state<{ driver: string; kind: 'twitch' | 'youtube' } | null>(
		null,
	);
	const selectedMedia = $derived(
		media?.driver === player.lfs_username ? media.kind : null,
	);
	function selectMedia(kind: 'twitch' | 'youtube') {
		media = { driver: player.lfs_username, kind };
	}
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<div class="mx-auto w-full max-w-screen-2xl px-4 py-4 md:px-6 md:py-3">
		<Breadcrumbs items={data.breadcrumbs} />
	</div>
	<PageIdentity label="Driver identity">
		<div class="space-y-4 py-2">
			<div
				class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
			>
				<div class="min-w-0 flex-1 space-y-2">
					<div class="flex items-center gap-3">
						<Flag
							code={player.flag_code}
							fallback={player.country_code}
							size="lg"
						/>
						<h1 class="racing-page-title min-w-0 break-words">
							{player.display_name}
						</h1>
					</div>
					<div
						class="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-muted-foreground"
					>
						<span class="break-all font-medium">{player.lfs_username}</span>
						{#each player.badges as badge (badge.id)}<PlayerBadge
								{badge}
							/>{/each}
					</div>
				</div>
				<div class="flex shrink-0 items-center gap-2">
					{#if ownProfile}<Button
							href="/account/personal"
							variant="outline"
							size="sm"><Pencil /> Edit profile</Button
						>{/if}
					<CompareButton driver={player} />
				</div>
			</div>
			{#if player.stats.first_hotlap_at}
				<p class="text-sm text-muted-foreground">
					First recorded hotlap <DateTime
						value={player.stats.first_hotlap_at}
					/>
				</p>
			{/if}
			{#if links.twitch_channel || links.youtube_channel || links.youtube_video_id || links.website_url}
				<div
					class="flex flex-wrap items-center gap-2"
					aria-label="Driver links"
				>
					{#if links.twitch_channel}
						<div
							class="inline-flex items-center overflow-hidden rounded-lg border border-purple-500/25 bg-purple-500/5 text-purple-700 dark:text-purple-300"
						>
							<Button
								href={`https://www.twitch.tv/${links.twitch_channel}`}
								target="_blank"
								rel="ugc nofollow noopener noreferrer"
								variant="ghost"
								size="sm"
								class="rounded-none text-inherit"
								><BrandIcon name="twitch" /> Twitch</Button
							>
							<Button
								variant="ghost"
								size="sm"
								class="rounded-none border-l border-purple-500/25 text-inherit"
								aria-expanded={selectedMedia === 'twitch'}
								aria-controls="driver-media"
								aria-haspopup="dialog"
								onclick={() =>
									selectedMedia === 'twitch'
										? (media = null)
										: selectMedia('twitch')}
								><Play class="size-3" /> Watch</Button
							>
						</div>
					{/if}
					{#if links.youtube_channel || links.youtube_video_id}
						<div
							class="inline-flex items-center overflow-hidden rounded-lg border border-red-500/25 bg-red-500/5 text-red-700 dark:text-red-300"
						>
							{#if links.youtube_channel}<Button
									href={links.youtube_channel}
									target="_blank"
									rel="ugc nofollow noopener noreferrer"
									variant="ghost"
									size="sm"
									class="rounded-none text-inherit"
									><BrandIcon name="youtube" /> YouTube</Button
								>{/if}
							{#if links.youtube_video_id}<Button
									variant="ghost"
									size="sm"
									class={links.youtube_channel
										? 'rounded-none border-l border-red-500/25 text-inherit'
										: 'text-inherit'}
									aria-expanded={selectedMedia === 'youtube'}
									aria-controls="driver-media"
									aria-haspopup="dialog"
									onclick={() =>
										selectedMedia === 'youtube'
											? (media = null)
											: selectMedia('youtube')}
									><Play class="size-3" /> Featured video</Button
								>{/if}
						</div>
					{/if}
					{#if links.website_url}<Button
							href={links.website_url}
							target="_blank"
							rel="ugc nofollow noopener noreferrer"
							variant="outline"
							size="sm">Website</Button
						>{/if}
				</div>
			{/if}
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
		<HotlapActivity
			title="Driver record"
			lfsUsername={player.lfs_username}
			eras={data.eras}
			refreshKey={player}
			views={['by_era', 'all', 'world_records']}
			defaultView="by_era"
			appearance="data"
		>
			{#snippet eraBreakdown()}
				{#if player.eras.length}
					<section aria-label="Record by era">
						<TableFrame appearance="data">
							<Table.Root
								class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
							>
								<Table.Header>
									<Table.Row>
										<Table.Head>Era</Table.Head>
										<Table.Head class="text-right">Hotlaps</Table.Head>
										<Table.Head class="text-right">Personal bests</Table.Head>
										<Table.Head class="text-right">1st place</Table.Head>
										<Table.Head class="text-right">2nd place</Table.Head>
										<Table.Head class="text-right">3rd place</Table.Head>
										<Table.Head>Badges</Table.Head>
										<Table.Head>Last lap</Table.Head>
									</Table.Row>
								</Table.Header>
								<Table.Body>
									{#each player.eras as era (era.id)}
										<Table.Row>
											<Table.Cell class="font-medium">
												<a
													class="hover:underline"
													href="/hotlaps/{era.id}/rankings">{era.title}</a
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
												<DateTime value={era.latest_hotlap_at} />
											</Table.Cell>
										</Table.Row>
									{/each}
								</Table.Body>
							</Table.Root>
						</TableFrame>
					</section>
				{:else}
					<Empty title="No hotlap record yet"
						><p>This driver has no recorded hotlaps.</p></Empty
					>
				{/if}
			{/snippet}
		</HotlapActivity>
	</div>
</main>

<DriverMedia {links} selected={selectedMedia} onClose={() => (media = null)} />
