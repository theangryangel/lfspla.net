<script lang="ts">
	import type { Snippet } from 'svelte';
	import { useFloatingPanels } from '$lib/floating-panels.svelte.js';

	let {
		order = 0,
		placement = 'bottom',
		children,
	}: {
		order?: number;
		placement?: 'bottom' | 'top-right';
		children: Snippet;
	} = $props();
	const panels = useFloatingPanels();
	const id = Symbol('panel');

	$effect(() => {
		panels.set(id, { order, placement, content: children });
		return () => {
			panels.delete(id);
		};
	});
</script>
