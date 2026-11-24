<script lang="ts">
	import { page } from '$app/state';
	import { afterNavigate } from '$app/navigation';
	import { exploreLinks } from './navigation.js';
	import type { EraSummary } from '#lib/api.js';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import HotlapMenuContent from './HotlapMenuContent.svelte';
	import DestinationMenuContent from './DestinationMenuContent.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	let { eras }: { eras: EraSummary[] } = $props();
	type Menu = 'hotlaps' | 'online' | 'explore';
	let openMenu = $state<Menu | null>(null);
	let activeTrigger: HTMLElement | null = null;
	const menus: { id: Menu; label: string }[] = [
		{ id: 'hotlaps', label: 'Hotlaps & Drag' },
		{ id: 'online', label: 'Online' },
		{ id: 'explore', label: 'Explore' },
	];
	afterNavigate(closeMenu);
	function closeMenu() {
		openMenu = null;
	}
	function dismiss(event: PointerEvent | FocusEvent) {
		if (
			event.target instanceof Element &&
			!event.target.closest('[data-global-mega-menu]')
		)
			closeMenu();
	}
	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && openMenu) {
			closeMenu();
			activeTrigger?.focus();
		}
	}
	function toggle(menu: Menu, event: MouseEvent) {
		activeTrigger = event.currentTarget as HTMLElement;
		openMenu = openMenu === menu ? null : menu;
	}
	function isActive(path: string) {
		return (
			page.url.pathname === path || page.url.pathname.startsWith(path + '/')
		);
	}
	function menuActive(menu: Menu) {
		return menu === 'hotlaps'
			? isActive('/hotlaps')
			: menu === 'online'
				? isActive('/zinc') || isActive('/events')
				: exploreLinks.some((link) => isActive(link.href));
	}
	const triggerClass = 'global-nav-trigger cursor-pointer text-sm font-medium';
	const panelClass =
		'dark absolute left-1/2 top-full z-50 max-h-[calc(100dvh-6rem)] w-screen -translate-x-1/2 overflow-y-auto border-y border-white/15 bg-[#111113] text-foreground shadow-lg';
</script>

<svelte:window
	onpointerdown={dismiss}
	onfocusin={dismiss}
	onkeydown={onKeydown}
/>
<nav
	aria-label="Global"
	class="global-navigation z-50 hidden items-center gap-2 md:flex"
>
	{#each menus as menu (menu.id)}
		<div
			data-global-mega-menu
			class={menu.id === 'hotlaps' ? 'relative' : undefined}
		>
			<Button
				id={`${menu.id}-menu-trigger`}
				variant="ghost"
				aria-expanded={openMenu === menu.id}
				aria-controls={`${menu.id}-mega-menu`}
				data-active={menuActive(menu.id) || openMenu === menu.id
					? ''
					: undefined}
				class={triggerClass}
				onclick={(event) => toggle(menu.id, event)}
			>
				{menu.label}<ChevronDownIcon
					class={`size-4 transition-transform ${openMenu === menu.id ? 'rotate-180' : ''}`}
				/>
			</Button>
			{#if openMenu === menu.id}
				<div
					id={`${menu.id}-mega-menu`}
					aria-labelledby={`${menu.id}-menu-trigger`}
					class={menu.id === 'hotlaps'
						? 'dark absolute left-0 top-full z-50 mt-2 max-h-[calc(100dvh-6rem)] w-64 overflow-y-auto rounded-lg border border-white/15 bg-[#111113] text-foreground shadow-lg'
						: panelClass}
				>
					{#if menu.id === 'hotlaps'}<HotlapMenuContent
							{eras}
							onNavigate={closeMenu}
						/>
					{:else}<DestinationMenuContent
							section={menu.id}
							onNavigate={closeMenu}
						/>{/if}
				</div>
			{/if}
		</div>
	{/each}
</nav>

<style>
	.global-navigation :global(.global-nav-trigger:hover),
	.global-navigation :global(.global-nav-trigger:focus-visible) {
		background-color: var(--color-accent);
		color: var(--color-accent-foreground);
	}
	.global-navigation :global(.global-nav-trigger[data-active]),
	.global-navigation :global(.global-nav-trigger[data-active]:hover),
	.global-navigation :global(.global-nav-trigger[data-active]:focus-visible) {
		background-color: var(--color-brand-yellow);
		color: #000;
	}
</style>
