<script lang="ts">
	import GlobeIcon from '@lucide/svelte/icons/globe';
	import BrandIcon from './BrandIcon.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { teamWebsite, type Team } from '#lib/teams.js';
	let { team }: { team: Team } = $props();
</script>

<div class="flex items-center gap-1" aria-label="Team links">
	{#if teamWebsite(team.website_url)}<Button
			href={teamWebsite(team.website_url)}
			target="_blank"
			rel="noopener noreferrer"
			variant="ghost"
			size="icon-sm"
			aria-label={`Website for ${team.name}`}
			title="Website"><GlobeIcon /></Button
		>{/if}
	{#if teamWebsite(team.discord_url)}<Button
			href={teamWebsite(team.discord_url)}
			target="_blank"
			rel="noopener noreferrer"
			variant="ghost"
			size="icon-sm"
			aria-label={`Discord for ${team.name}`}
			title="Discord"><BrandIcon name="discord" /></Button
		>{/if}
</div>
