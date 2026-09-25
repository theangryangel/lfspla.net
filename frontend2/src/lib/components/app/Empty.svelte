<script lang="ts">
	import type { Snippet } from 'svelte';
	import InboxIcon from '@lucide/svelte/icons/inbox';
	import * as Card from '$lib/components/ui/card/index.js';

	let { title, children }: { title: string; children?: Snippet } = $props();
</script>

<Card.Root>
	<Card.Content class="flex flex-col items-center gap-4 py-12 text-center">
		<InboxIcon class="size-8 text-muted-foreground" />
		<h2 class="text-lg font-semibold">{title}</h2>
		<div class="space-y-4 text-sm text-muted-foreground">
			{@render children?.()}
		</div>
	</Card.Content>
</Card.Root>
