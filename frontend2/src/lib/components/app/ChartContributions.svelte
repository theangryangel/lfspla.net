<script lang="ts">
	import InfoIcon from '@lucide/svelte/icons/info';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import RankingList from './RankingList.svelte';
	import type { EraSummary, RankingContribution } from '$lib/api.js';

	let {
		contributions,
		era,
		track,
		vehicle,
	}: {
		contributions: RankingContribution[];
		era: EraSummary;
		track?: string;
		vehicle?: string;
	} = $props();
	let open = $state(false);
	const rankings = $derived(
		contributions.map((ranking) => ({
			...ranking,
			description: era.rankings.find((candidate) => candidate.id === ranking.id)
				?.description,
		})),
	);
</script>

<Dialog.Root bind:open>
	<Dialog.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="outline"
				size="sm"
				class="h-8 rounded-sm text-sm"
				aria-label="Contributes toward rankings"
				title="Rankings this chart contributes to"
			>
				<InfoIcon class="size-4" aria-hidden="true" />
				Contributes toward rankings…
			</Button>
		{/snippet}
	</Dialog.Trigger>
	<Dialog.Content
		class="max-h-[calc(100dvh-2rem)] overflow-y-auto rounded-sm sm:max-w-2xl"
	>
		<Dialog.Header class="pr-8">
			<Dialog.Title class="racing-section-title">Contributes to</Dialog.Title>
			<Dialog.Description
				>Personal bests on {track} / {vehicle} count towards these rankings in {era.title}.</Dialog.Description
			>
		</Dialog.Header>
		<RankingList
			era={era.id}
			{rankings}
			label="Contributing rankings"
			empty="No rankings include this chart."
			onChoose={(event) => {
				if (
					!event.metaKey &&
					!event.ctrlKey &&
					!event.shiftKey &&
					!event.altKey
				)
					open = false;
			}}
		/>
	</Dialog.Content>
</Dialog.Root>
