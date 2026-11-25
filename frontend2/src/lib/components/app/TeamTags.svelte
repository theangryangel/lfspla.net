<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { disciplineLabels } from '#lib/teams.js';
	import type { TeamSummary } from '#lib/api.js';

	let {
		team,
	}: { team: Pick<TeamSummary, 'tags' | 'accepts_applications' | 'status'> } =
		$props();
	const open = $derived(team.accepts_applications && team.status === 'active');
</script>

<div class="flex flex-wrap items-center gap-1.5">
	{#each team.tags as tag}<Badge variant="secondary"
			>{disciplineLabels[tag]}</Badge
		>{/each}
	<Badge variant="outline" class={open ? 'border-time-pb/40 text-time-pb' : ''}
		>{open ? 'Applications open' : 'Applications closed'}</Badge
	>
	{#if team.status === 'disbanded'}<Badge variant="outline">Disbanded</Badge
		>{/if}
</div>
