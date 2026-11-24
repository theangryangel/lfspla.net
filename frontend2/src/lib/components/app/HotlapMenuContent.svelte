<script lang="ts">
	import { page } from '$app/state';
	import type { EraSummary } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		eras,
		onNavigate = () => {},
	}: { eras: EraSummary[]; onNavigate?: () => void } = $props();
	const groups = $derived(
		[
			{
				label: 'Open',
				eras: [...eras].reverse().filter((era) => era.open),
			},
			{
				label: 'Historical',
				eras: [...eras].reverse().filter((era) => !era.open),
			},
		].filter((group) => group.eras.length),
	);
</script>

<div class="era-menu space-y-4 p-3">
	{#each groups as group (group.label)}
		<section>
			<h2 class="mb-1 px-2.5 text-xs font-semibold text-muted-foreground">
				{group.label}
			</h2>
			<div class="space-y-1">
				{#each group.eras as era (era.id)}
					<Button
						variant="ghost"
						href={`/hotlaps/${era.id}`}
						onclick={onNavigate}
						aria-current={page.params.era === era.id ? 'page' : undefined}
						class="w-full justify-start">{era.title}</Button
					>
				{/each}
			</div>
		</section>
	{/each}
	{#if !eras.length}<p class="px-2.5 text-sm text-muted-foreground">
			No hotlap eras available.
		</p>{/if}
</div>

<style>
	.era-menu :global([data-slot='button']:hover),
	.era-menu :global([data-slot='button']:focus-visible),
	.era-menu :global([data-slot='button'][aria-current='page']) {
		background-color: var(--color-brand-yellow);
		color: #000;
	}
</style>
