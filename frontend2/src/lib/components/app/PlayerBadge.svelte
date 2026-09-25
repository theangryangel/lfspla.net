<script lang="ts">
	import gabor from '$assets/badges/gabor.svg?raw';
	import mhr from '$assets/badges/mhr.svg?raw';
	import nutter from '$assets/badges/nutter.svg?raw';
	import newbie from '$assets/badges/newbie.svg?raw';
	import ranking from '$assets/badges/ranking.svg?raw';
	import worldRecordLeader from '$assets/badges/wr.svg?raw';
	import type {
		PlayerBadge as PlayerBadgeData,
		PodiumLevel,
	} from '$lib/api.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { cn } from '$lib/utils.js';

	let {
		badge,
		class: className = '',
	}: { badge: PlayerBadgeData; class?: string } = $props();

	type Presentation = { artwork: string; colours: string; title: string };

	// Unknown rankings use generic artwork.
	const rankingArtwork: Record<string, string | undefined> = { mhr, nutter };
	const podiumLevels: Record<number, PodiumLevel | undefined> = {
		1: 'gold',
		2: 'silver',
		3: 'bronze',
	};
	const podiumColours: Record<PodiumLevel, string> = {
		gold: 'text-[#e8bb20] dark:text-[#f4cd38]',
		silver: 'text-[#b8bec7] dark:text-[#d1d5db]',
		bronze: 'text-[#bf8045] dark:text-[#d99a5e]',
	};
	const neutralColours = 'text-slate-400 dark:text-slate-300';

	function positionColours(position: number): string {
		const level = podiumLevels[position];
		return level ? podiumColours[level] : neutralColours;
	}

	function capitalise(value: string): string {
		return `${value[0].toUpperCase()}${value.slice(1)}`;
	}

	function pluralise(count: number, noun: string): string {
		return `${count} ${noun}${count === 1 ? '' : 's'}`;
	}

	function present(badge: PlayerBadgeData): Presentation {
		switch (badge.kind) {
			case 'ranking':
				return {
					artwork: rankingArtwork[badge.ranking_id] ?? ranking,
					colours: positionColours(badge.ranking_position),
					title: `${badge.title} - #${badge.ranking_position}`,
				};
			case 'ranking_completion':
				return {
					artwork: badge.ranking_id === 'nutter' ? gabor : ranking,
					colours: neutralColours,
					title: `${badge.label} - ${badge.title} 100% complete`,
				};
			case 'world_record_podium':
				return {
					artwork: ranking,
					colours: podiumColours[badge.level],
					title: `${capitalise(badge.level)} - ${badge.firsts} first, ${badge.seconds} second, ${badge.thirds} third places`,
				};
			case 'world_record_leader':
				return {
					artwork: worldRecordLeader,
					colours: positionColours(badge.leader_position),
					title: `World records - #${badge.leader_position} with ${pluralise(badge.world_records, 'world record')}`,
				};
			case 'newbie':
				return {
					artwork: newbie,
					colours: 'text-[#8bd629] dark:text-[#9ae238]',
					title: 'Newbie - one published hotlap in this era',
				};
		}
		// Check new badge kinds at build time.
		return badge satisfies never;
	}

	const presentation = $derived(present(badge));
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		<!-- Badges are not tab stops; the sr-only label provides their text. -->
		{#snippet child({ props })}
			<span
				{...props}
				class={cn(
					'inline-flex shrink-0 items-center gap-1 whitespace-nowrap align-middle text-xs font-medium',
					className,
				)}
			>
				<span class="sr-only">{presentation.title}</span>
				<span
					class={cn(
						'inline-flex size-5 shrink-0 [&>svg]:size-full',
						presentation.colours,
					)}
					aria-hidden="true"
				>
					{@html presentation.artwork}
				</span>
			</span>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content>{presentation.title}</Tooltip.Content>
</Tooltip.Root>
