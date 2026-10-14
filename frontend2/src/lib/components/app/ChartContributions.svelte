<script lang="ts">
	import InfoIcon from '@lucide/svelte/icons/info';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
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
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Contributes to</Dialog.Title>
			<Dialog.Description
				>Personal bests on {track} / {vehicle} count towards these rankings in {era.title}.</Dialog.Description
			>
		</Dialog.Header>
		<div class="flex max-h-[60vh] flex-wrap gap-2 overflow-auto">
			{#each contributions as ranking (ranking.id)}
				<Badge
					variant="outline"
					href={`/hotlaps/${era.id}/rankings/${encodeURIComponent(ranking.id)}`}
					onclick={() => (open = false)}>{ranking.title}</Badge
				>
			{:else}
				<p class="text-sm text-muted-foreground">
					No rankings include this chart.
				</p>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>
