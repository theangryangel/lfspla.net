<script lang="ts">
	import DestinationMenuContent from './DestinationMenuContent.svelte';
	import HotlapMenuContent from './HotlapMenuContent.svelte';
	import type { EraSummary } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { Session } from '#lib/session.svelte.js';
	import MenuIcon from '@lucide/svelte/icons/menu';
	import XIcon from '@lucide/svelte/icons/x';

	let { eras, session }: { eras: EraSummary[]; session: Session } = $props();
	let mobileOpen = $state(false);

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
			class="dark absolute left-0 right-0 top-full z-50 max-h-[80dvh] overflow-y-auto border-y bg-background p-6 text-foreground shadow-lg"
		>
			<details open class="-mx-6 border-b border-white/10 pb-4">
				<summary class="cursor-pointer px-6 text-base font-semibold"
					>Hotlaps &amp; Drag</summary
				>
				<HotlapMenuContent {eras} onNavigate={closeMobileMenu} />
			</details>
			<details class="-mx-6 border-b border-white/10 py-4">
				<summary class="cursor-pointer px-6 text-base font-semibold"
					>Online</summary
				>
				<DestinationMenuContent section="online" onNavigate={closeMobileMenu} />
			</details>
			<details class="-mx-6 border-y border-white/10 py-4">
				<summary class="cursor-pointer px-6 text-base font-semibold"
					>Explore</summary
				>
				<DestinationMenuContent
					section="explore"
					onNavigate={closeMobileMenu}
				/>
			</details>
			<div class="mt-3 pl-2">
				<h2 class="text-sm font-semibold tracking-wide text-muted-foreground">
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
