<script lang="ts">
	import * as Select from '$lib/components/ui/select/index.js';

	let {
		label,
		value,
		options,
		onValueChange,
	}: {
		label: string;
		value: string;
		options: { value: string; label: string }[];
		onValueChange: (value: string) => void;
	} = $props();

	// bits-ui reserves the empty string for no selection.
	const ALL = '__all__';
	const encode = (v: string) => v || ALL;
	const decode = (v: string) => (v === ALL ? '' : v);

	const current = $derived(options.find((o) => o.value === value)?.label ?? '');
</script>

<Select.Root
	type="single"
	value={encode(value)}
	onValueChange={(v) => {
		if (v) onValueChange(decode(v));
	}}
>
	<Select.Trigger aria-label={label}>{current}</Select.Trigger>
	<Select.Content>
		{#each options as option (option.value)}
			<Select.Item value={encode(option.value)} label={option.label} />
		{/each}
	</Select.Content>
</Select.Root>
