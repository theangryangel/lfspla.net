<script lang="ts">
	import type { GameConfig } from '#lib/api.js';

	let { game }: { game: GameConfig } = $props();
</script>

<section class="min-w-0 space-y-3 text-sm" aria-label="Event settings">
	{#if game.source.type === 'bomb'}
		<dl class="flex flex-wrap gap-x-8 gap-y-3 text-sm">
			<div>
				<dt class="text-muted-foreground">Checkpoint timer</dt>
				<dd class="font-mono tabular-nums">
					{game.source.checkpoint_timeout_secs}s
				</dd>
			</div>
			<div>
				<dt class="text-muted-foreground">Penalty per checkpoint</dt>
				<dd class="font-mono tabular-nums">
					{game.source.checkpoint_penalty_ms}ms
				</dd>
			</div>
			<div>
				<dt class="text-muted-foreground">Maximum collision penalty</dt>
				<dd class="font-mono tabular-nums">
					{game.source.collision_max_penalty_ms}ms
				</dd>
			</div>
		</dl>
	{/if}
</section>
