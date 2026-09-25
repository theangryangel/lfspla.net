<script lang="ts" module>
	import type { Snippet } from 'svelte';
	interface NavigationLinkItem {
		key?: string;
		content?: never;
		label: string;
		to: string;
		match?: string;
		exact?: boolean;
	}
	export type NavigationItem =
		| NavigationLinkItem
		| {
				key: string;
				content: Snippet;
				label: string;
				to?: never;
		  };
</script>

<script lang="ts">
	import { page } from '$app/state';
	import { navigationTextClass } from './navigation.js';

	let {
		label,
		items,
		prominent = false,
		action,
	}: {
		label: string;
		items: NavigationItem[];
		prominent?: boolean;
		action?: Snippet;
	} = $props();

	function isActive(item: NavigationLinkItem) {
		const path = item.match || item.to;
		return item.exact
			? page.url.pathname === path
			: page.url.pathname === path || page.url.pathname.startsWith(path + '/');
	}
</script>

<nav
	aria-label={label}
	class="w-full max-w-full justify-start overflow-x-auto md:w-auto md:max-w-max"
>
	<ul
		class="flex min-w-max list-none flex-nowrap items-center justify-start gap-2"
	>
		{#each items as item (item.key ?? item.to)}
			<li>
				{#if item.content}
					{@render item.content()}
				{:else}
					<a
						href={item.to}
						aria-current={isActive(item) ? 'page' : undefined}
						data-active={isActive(item) ? '' : undefined}
						class={prominent
							? 'flex items-center text-sm transition-colors focus-visible:outline-2 focus-visible:outline-ring rounded-none border-b-2 border-transparent px-3 py-2 font-semibold text-muted-foreground hover:text-foreground data-active:border-primary data-active:bg-transparent data-active:text-foreground data-active:hover:bg-transparent data-active:focus:bg-transparent'
							: `flex items-center rounded-lg transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-2 focus-visible:outline-ring h-8 px-2.5 py-0 ${navigationTextClass}`}
					>
						{item.label}
					</a>
				{/if}
			</li>
		{/each}
		{#if action}
			<li class="ml-2">
				{@render action()}
			</li>
		{/if}
	</ul>
</nav>
