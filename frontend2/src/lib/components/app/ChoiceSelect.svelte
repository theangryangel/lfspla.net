<script lang="ts">
	import * as Select from '#lib/components/ui/select/index.js';

	let {
		label,
		value,
		options,
		class: className,
		disabled = false,
		onValueChange,
	}: {
		label: string;
		value: string;
		options: { value: string; label: string }[];
		class?: string;
		disabled?: boolean;
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
	{disabled}
	value={encode(value)}
	onValueChange={(v) => {
		if (v) onValueChange(decode(v));
	}}
>
	<Select.Trigger aria-label={label} class={className}
		><span class="truncate">{current}</span></Select.Trigger
	>
	<Select.Content>
		{#each options as option (option.value)}
			<Select.Item value={encode(option.value)} label={option.label} />
		{/each}
	</Select.Content>
</Select.Root>
