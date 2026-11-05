<script lang="ts">
	import type { Hotlap } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import InfoIcon from '@lucide/svelte/icons/info';
	import DownloadIcon from '@lucide/svelte/icons/download';
	let { hotlap }: { hotlap: Pick<Hotlap, 'id' | 'spr' | 'raf'> } = $props();
</script>

<div class="flex w-max items-center gap-1">
	<Button
		variant="ghost"
		size="icon-sm"
		href={`/hotlap/${hotlap.id}`}
		aria-label={`Details for hotlap #${hotlap.id}`}
		title="Hotlap details"><InfoIcon aria-hidden="true" /></Button
	>
	{#if hotlap.spr}
		<Button
			variant="ghost"
			size="icon-sm"
			href={hotlap.spr.url}
			aria-label={`Download SPR replay for hotlap #${hotlap.id}`}
			title="Download SPR replay"><DownloadIcon aria-hidden="true" /></Button
		>
	{/if}
	{#if hotlap.raf}
		<Button
			variant="ghost"
			size="sm"
			href={hotlap.raf.url}
			aria-label={`Download RAF telemetry for hotlap #${hotlap.id}`}
			title="Download RAF telemetry">RAF</Button
		>
	{/if}
</div>
