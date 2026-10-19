<script lang="ts">
	import type { PlayerBadge as PlayerBadgeData } from '$lib/api.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { cn } from '$lib/utils.js';

	let {
		badge,
		class: className = '',
	}: { badge: PlayerBadgeData; class?: string } = $props();

	const title = $derived(`${badge.name} - ${badge.description}`);
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			<span
				{...props}
				class={cn(
					'inline-flex shrink-0 items-center gap-1 whitespace-nowrap align-middle text-xs font-medium',
					className,
				)}
			>
				<span class="sr-only">{title}</span>
				{#if badge.image}
					<img
						src={badge.image}
						alt=""
						class="size-5 shrink-0 object-contain"
					/>
				{:else}
					<span aria-hidden="true">{badge.name}</span>
				{/if}
			</span>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content>{title}</Tooltip.Content>
</Tooltip.Root>
