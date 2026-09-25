<script lang="ts" module>
	import { tv } from 'tailwind-variants';
	import { cn } from '$lib/utils.js';

	export const navigationMenuTriggerStyle = tv({
		base: 'rounded-lg px-2.5 py-1.5 text-sm font-medium transition-all hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-1 disabled:opacity-50 data-active:bg-accent data-active:text-accent-foreground data-popup-open:bg-accent data-popup-open:text-accent-foreground data-popup-open:hover:bg-accent data-popup-open:hover:text-accent-foreground data-open:bg-accent data-open:text-accent-foreground data-open:hover:bg-accent data-open:hover:text-accent-foreground data-open:focus:bg-accent data-open:focus:text-accent-foreground group/navigation-menu-trigger inline-flex h-9 w-max items-center justify-center outline-none disabled:pointer-events-none',
	});
</script>

<script lang="ts">
	import { NavigationMenu as NavigationMenuPrimitive } from 'bits-ui';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: NavigationMenuPrimitive.TriggerProps = $props();
</script>

<NavigationMenuPrimitive.Trigger
	bind:ref
	data-slot="navigation-menu-trigger"
	class={cn(navigationMenuTriggerStyle(), 'group', className)}
	{...restProps}
>
	{@render children?.()}
	<ChevronDownIcon
		class="relative top-px ml-1 size-3 transition duration-300 group-data-popup-open/navigation-menu-trigger:rotate-180 group-data-open/navigation-menu-trigger:rotate-180"
		aria-hidden="true"
	/>
</NavigationMenuPrimitive.Trigger>
