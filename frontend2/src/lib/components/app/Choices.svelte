<script lang="ts">
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';

	let {
		label,
		value,
		options,
		onValueChange,
	}: {
		label: string;
		value: string;
		options: string[];
		onValueChange: (value: string) => void;
	} = $props();
</script>

<ToggleGroup.Root
	type="single"
	aria-label={label}
	variant="outline"
	{value}
	onValueChange={(v) => {
		if (v) onValueChange(v);
	}}
>
	{#each options as option (option)}
		<ToggleGroup.Item value={option}>{option}</ToggleGroup.Item>
	{/each}
</ToggleGroup.Root>
