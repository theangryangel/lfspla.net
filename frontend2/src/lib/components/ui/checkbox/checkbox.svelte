<script lang="ts">
	import { Checkbox as Primitive } from 'bits-ui';
	import Check from '@lucide/svelte/icons/check';
	import Minus from '@lucide/svelte/icons/minus';
	import { cn } from '#lib/utils.js';
	let {
		checked = $bindable(false),
		indeterminate = $bindable(false),
		ref = $bindable(null),
		class: className,
		...props
	}: Primitive.RootProps = $props();
</script>

<Primitive.Root
	bind:checked
	bind:indeterminate
	bind:ref
	data-slot="checkbox"
	class={cn(
		'inline-flex size-4 shrink-0 cursor-pointer items-center justify-center rounded border border-input bg-background text-primary-foreground outline-none transition-colors data-[state=checked]:border-primary data-[state=checked]:bg-primary data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50',
		className,
	)}
	{...props}
>
	{#if indeterminate}<Minus
			class="size-3"
			aria-hidden="true"
		/>{:else if checked}<Check
			class="size-3"
			strokeWidth={3}
			aria-hidden="true"
		/>{/if}
</Primitive.Root>
