<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	let {
		label,
		children,
		disabled = false,
	}: { label: string; children: Snippet; disabled?: boolean } = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}<Button
				{...props}
				variant="ghost"
				size="icon-sm"
				aria-label={label}
				title={label}
				{disabled}><EllipsisIcon class="size-4" /></Button
			>{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end">{@render children()}</DropdownMenu.Content>
</DropdownMenu.Root>
