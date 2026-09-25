<script lang="ts">
	import { page } from '$app/state';
	import type { EraSummary } from '$lib/api.js';
	import { hotlapPath } from '$lib/era.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import type { Session } from '$lib/session.svelte.js';
	import MenuIcon from '@lucide/svelte/icons/menu';
	import XIcon from '@lucide/svelte/icons/x';

	let { eras, session }: { eras: EraSummary[]; session: Session } = $props();
	const newestFirst = $derived([...eras].reverse());
	const eraGroups = $derived(
		[
			{ label: 'Open Hotlaps', eras: newestFirst.filter((era) => era.open) },
			{
				label: 'Historical Hotlaps',
				eras: newestFirst.filter((era) => !era.open),
			},
		].filter((group) => group.eras.length > 0),
	);
	let mobileOpen = $state(false);

	function isActive(path: string) {
		return (
			page.url.pathname === path || page.url.pathname.startsWith(path + '/')
		);
	}

	function closeMobileMenu() {
		mobileOpen = false;
	}

	async function signOut() {
		closeMobileMenu();
		await session.signOut();
	}
</script>

<div class="md:hidden">
	<Button
		variant="outline"
		size="icon"
		aria-label={mobileOpen ? 'Close navigation menu' : 'Open navigation menu'}
		aria-expanded={mobileOpen}
		aria-controls="mobile-navigation"
		onclick={() => (mobileOpen = !mobileOpen)}
	>
		{#if mobileOpen}
			<XIcon class="size-4" aria-hidden="true" />
		{:else}
			<MenuIcon class="size-4" aria-hidden="true" />
		{/if}
	</Button>

	{#if mobileOpen}
		<nav
			id="mobile-navigation"
			aria-label="Global"
			class="dark absolute left-0 right-0 top-full z-50 border-y bg-background p-6 text-foreground shadow-lg"
		>
			<div class="space-y-3 pl-2">
				{#each eraGroups as group (group.label)}
					<section>
						<h2
							class="text-xs font-semibold tracking-wide text-muted-foreground"
						>
							{group.label}
						</h2>
						<div class="mt-1 border-l pl-3">
							{#each group.eras as era (era.id)}
								<a
									class={`block rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-accent hover:text-accent-foreground ${isActive(hotlapPath(era.id)) ? 'bg-accent text-accent-foreground' : ''}`}
									href={hotlapPath(era.id)}
									onclick={closeMobileMenu}
									aria-current={isActive(hotlapPath(era.id))
										? 'page'
										: undefined}
								>
									{era.title}
								</a>
							{/each}
						</div>
					</section>
				{/each}
			</div>
			<a
				class={`mt-3 flex h-9 items-center rounded-md px-2.5 text-sm font-medium transition-colors hover:bg-accent hover:text-accent-foreground ${isActive('/drivers') ? 'bg-accent text-accent-foreground' : ''}`}
				href="/drivers"
				onclick={closeMobileMenu}
				aria-current={isActive('/drivers') ? 'page' : undefined}
			>
				Drivers
			</a>
			<div class="mt-3 pl-2">
				<h2 class="text-xs font-semibold tracking-wide text-muted-foreground">
					My account
				</h2>
				<div class="mt-1 border-l pl-3">
					{#if session.signedIn}
						<a
							class="block rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
							href="/account/hotlaps"
							onclick={closeMobileMenu}>My hotlaps</a
						>
						<a
							class="block rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
							href="/account/personal"
							onclick={closeMobileMenu}>Personal settings</a
						>
						<a
							class="block rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
							href="/account/tokens"
							onclick={closeMobileMenu}>Personal access tokens</a
						>
						<a
							class="block rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
							href="/account/webhooks"
							onclick={closeMobileMenu}>Webhooks</a
						>
						<button
							class="block w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
							disabled={session.pending}
							onclick={signOut}>Sign out</button
						>
					{:else}
						<button
							class="block w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
							onclick={() => {
								closeMobileMenu();
								session.signIn();
							}}>Sign in with LFS</button
						>
					{/if}
				</div>
			</div>
		</nav>
	{/if}
</div>
