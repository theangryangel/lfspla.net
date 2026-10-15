<script lang="ts">
	import Progress from './Progress.svelte';
	let {
		era,
		rankings,
		selected,
		progress,
		onChoose,
		label = 'Available rankings',
		empty = 'No rankings in this era yet.',
	}: {
		era: string;
		rankings: { id: string; title: string; description?: string }[];
		selected?: string;
		progress?: Record<string, { completed: number; total: number }> | null;
		onChoose?: (event: MouseEvent) => void;
		label?: string;
		empty?: string;
	} = $props();
</script>

<ul class="max-h-[50dvh] overflow-y-auto border-y divide-y" aria-label={label}>
	{#each rankings as ranking (ranking.id)}
		<li
			class="grid gap-2 px-3 py-3 hover:bg-accent/40"
			class:sm:grid-cols-[minmax(0,1fr)_10rem]={!!progress}
			class:sm:items-center={!!progress}
			class:bg-accent={ranking.id === selected}
		>
			<div class="min-w-0">
				<a
					href={`/hotlaps/${era}/rankings/${encodeURIComponent(ranking.id)}`}
					onclick={onChoose}
					aria-current={ranking.id === selected ? 'page' : undefined}
					class="font-semibold hover:underline focus-visible:outline-2 focus-visible:outline-ring"
					>{ranking.title}</a
				>
				{#if ranking.description}<p class="mt-1 text-muted-foreground">
						{ranking.description}
					</p>{/if}
			</div>
			{#if progress?.[ranking.id]}
				{@const own = progress[ranking.id]}
				<a
					href={`/hotlaps/${era}/rankings/${encodeURIComponent(ranking.id)}/combinations`}
					onclick={onChoose}
					aria-label={`${ranking.title}: your progress`}
					class="focus-visible:outline-2 focus-visible:outline-ring"
					><Progress value={own.completed} total={own.total} /></a
				>
			{/if}
		</li>
	{:else}
		<li class="p-4 text-muted-foreground">{empty}</li>
	{/each}
</ul>
