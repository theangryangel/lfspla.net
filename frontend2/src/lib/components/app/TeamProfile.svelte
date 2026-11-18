<script lang="ts">
	import type { Team } from '#lib/teams.js';
	import TeamLinks from './TeamLinks.svelte';
	let { team }: { team: Team } = $props();
</script>

<header class="space-y-3">
	<h1 class="racing-page-title">{team.name}</h1>
	{#if !team.is_active}<p class="text-sm text-muted-foreground">
			Disbanded
		</p>{/if}
	{#if team.description}<p class="whitespace-pre-wrap text-muted-foreground">
			{team.description}
		</p>{/if}
	<TeamLinks {team} />
</header>
