<script lang="ts">
	import { NavigationMenu as NavigationMenuPrimitive } from 'bits-ui';
	import { cn } from '$lib/utils.js';

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: NavigationMenuPrimitive.LinkProps = $props();
</script>

<NavigationMenuPrimitive.Link
	bind:ref
	data-slot="navigation-menu-link"
	class={cn(
		"flex items-center gap-2 rounded-lg p-2 text-sm transition-all outline-none hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-1 in-data-[slot=navigation-menu-content]:rounded-md data-active:bg-accent data-active:text-accent-foreground data-active:hover:bg-accent data-active:hover:text-accent-foreground data-active:focus:bg-accent data-active:focus:text-accent-foreground [&_svg:not([class*='size-'])]:size-4",
		className,
	)}
	{...restProps}
/>
