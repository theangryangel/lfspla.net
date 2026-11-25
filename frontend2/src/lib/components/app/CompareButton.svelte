<script lang="ts">
	import GitCompareArrows from '@lucide/svelte/icons/git-compare-arrows';
	import { mergeProps } from 'bits-ui';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Tooltip from '#lib/components/ui/tooltip/index.js';
	import { useComparison, type CompareDriver } from '#lib/compare.svelte.js';
	let {
		driver,
		showLabel = false,
	}: { driver: CompareDriver; showLabel?: boolean } = $props();
	const comparison = useComparison();
	const selected = $derived(comparison.selected(driver.lfs_username));
	const full = $derived(!selected && comparison.drivers.length === 2);
	const label = $derived(
		selected ? 'Remove from driver comparison' : 'Add to driver comparison',
	);
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			<!--
				Merged rather than spread so the tooltip keeps the click handler that
				dismisses it when the button is pressed.
			-->
			<Button
				{...mergeProps(props, { onclick: () => comparison.toggle(driver) })}
				variant={selected || showLabel ? 'outline' : 'ghost'}
				size={showLabel ? 'sm' : 'icon-sm'}
				aria-pressed={selected}
				aria-label={label}
				disabled={full}
			>
				<GitCompareArrows />
				{#if showLabel}{selected ? 'In comparison' : 'Compare driver'}{/if}
			</Button>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content>{label}</Tooltip.Content>
</Tooltip.Root>
