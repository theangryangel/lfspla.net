<script lang="ts">
	import type { EraSummary, WorldRecordHolderResponse } from '$lib/api.js';
	import PageIdentity from './PageIdentity.svelte';
	import RecordHighlight from './RecordHighlight.svelte';
	import Flag from './Flag.svelte';
	let {
		era,
		leaders = [],
	}: { era: EraSummary; leaders?: WorldRecordHolderResponse[] } = $props();
</script>

<PageIdentity label="Era identity">
	<div class="space-y-3 py-2">
		<h1 class="racing-page-title">{era.title}</h1>
		<p class="max-w-prose text-sm text-muted-foreground">
			{#if era.open}
				This era is open for hotlap submissions and accepts replays recorded
				with LFS {era.version_requirement}.
			{:else}
				This historical era accepts replays recorded with LFS {era.version_requirement}.
			{/if}
		</p>
	</div>
	{#snippet highlight()}
		{#if leaders.length}
			<RecordHighlight title="Most world records" variant="purple">
				<p class="text-right text-3xl font-bold tabular-nums">
					{leaders[0].world_records.toLocaleString()}
				</p>
				<div class="grid gap-1 text-sm">
					{#each leaders as holder (holder.player.id)}
						<a
							class="flex min-w-0 items-center justify-end gap-2 font-medium hover:underline"
							href="/drivers/{encodeURIComponent(holder.player.lfs_username)}"
						>
							<Flag
								code={holder.player.flag_code}
								fallback={holder.player.country_code}
							/>
							<span class="break-words">{holder.player.display_name}</span>
						</a>
					{/each}
				</div>
			</RecordHighlight>
		{/if}
	{/snippet}
</PageIdentity>
