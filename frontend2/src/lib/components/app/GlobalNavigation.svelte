<script lang="ts">
	import { page } from '$app/state';
	import { navigationTextClass } from './navigation.js';
	import type { EraSummary } from '$lib/api.js';
	import { hotlapPath } from '$lib/era.js';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import * as NavigationMenu from '$lib/components/ui/navigation-menu/index.js';

	let { eras }: { eras: EraSummary[] } = $props();
	const newestFirst = $derived([...eras].reverse());
	const eraGroups = $derived(
		[
			{ label: 'Open', eras: newestFirst.filter((era) => era.open) },
			{ label: 'Historical', eras: newestFirst.filter((era) => !era.open) },
		].filter((group) => group.eras.length > 0),
	);
	const linkClass = `h-8 px-2.5 py-0 font-medium ${navigationTextClass}`;
	function isActive(path: string) {
		return (
			page.url.pathname === path || page.url.pathname.startsWith(path + '/')
		);
	}
</script>

<NavigationMenu.Root
	aria-label="Global"
	class="z-50 hidden md:flex"
	viewport={false}
>
	<NavigationMenu.List class="gap-2">
		<NavigationMenu.Item>
			<DropdownMenu.Root>
				<DropdownMenu.Trigger>
					{#snippet child({ props })}
						<Button
							{...props}
							variant="ghost"
							data-active={isActive('/hotlaps') ? '' : undefined}
			class={`${navigationTextClass} !cursor-pointer`}
						>
							Hotlaps
							<ChevronDownIcon class="size-4" />
						</Button>
					{/snippet}
				</DropdownMenu.Trigger>
				<DropdownMenu.Content
					align="start"
					class="dark max-h-[65vh] w-64 max-w-[calc(100vw-2rem)]"
				>
					{#each eraGroups as group, groupIndex (group.label)}
						<DropdownMenu.Group>
							<DropdownMenu.GroupHeading
								>{group.label}</DropdownMenu.GroupHeading
							>
							<div class="ml-2 border-l pl-2.5">
								{#each group.eras as era, eraIndex (era.id)}
									<DropdownMenu.Item
										class={`!cursor-pointer data-active:bg-accent data-active:text-accent-foreground ${groupIndex === eraGroups.length - 1 && eraIndex === group.eras.length - 1 ? 'mb-1' : ''}`}
										data-active={isActive(hotlapPath(era.id)) ? '' : undefined}
									>
										{#snippet child({ props })}
											<a
												{...props}
												class={`${props.class ?? ''} !cursor-pointer`}
												href={hotlapPath(era.id)}
												aria-current={isActive(hotlapPath(era.id))
													? 'page'
													: undefined}
											>
												{era.title}
											</a>
										{/snippet}
									</DropdownMenu.Item>
								{/each}
							</div>
						</DropdownMenu.Group>
					{/each}
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</NavigationMenu.Item>
		{#each [{ label: 'Drivers', href: '/drivers' }] as item}
			<NavigationMenu.Item>
				<NavigationMenu.Link
					href={item.href}
					active={isActive(item.href)}
					class={linkClass}>{item.label}</NavigationMenu.Link
				>
			</NavigationMenu.Item>
		{/each}
	</NavigationMenu.List>
</NavigationMenu.Root>
