<script lang="ts">
	import '../app.css';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import MoonIcon from '@lucide/svelte/icons/moon';
	import SunIcon from '@lucide/svelte/icons/sun';
	import { onMount } from 'svelte';
	import { mode, ModeWatcher, toggleMode } from 'mode-watcher';
	import { setComparison } from '$lib/compare.svelte.js';
	import { setFloatingPanels } from '$lib/floating-panels.svelte.js';
	import FloatingPanelStack from '$lib/components/app/FloatingPanelStack.svelte';
	import CompareTray from '$lib/components/app/CompareTray.svelte';
	import type { Snippet } from 'svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import UserRoundIcon from '@lucide/svelte/icons/user-round';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import Flag from '$lib/components/app/Flag.svelte';
	import GlobalNavigation from '$lib/components/app/GlobalNavigation.svelte';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import MobileNavigation from '$lib/components/app/MobileNavigation.svelte';
	import Wordmark from '$lib/components/app/Wordmark.svelte';
	import { setSession } from '$lib/session.svelte.js';
	import type { LayoutProps } from './$types';

	let { data, children }: LayoutProps & { children: Snippet } = $props();

	const comparison = setComparison();
	setFloatingPanels();
	onMount(() => {
		comparison.restore();
		window.addEventListener('storage', comparison.storage);
		return () => window.removeEventListener('storage', comparison.storage);
	});

	const session = setSession(() => data.me);
</script>

<svelte:head>
	<title>lfspla.net</title>
</svelte:head>

<ModeWatcher />

<Tooltip.Provider delayDuration={300}>
	<div class="flex min-h-screen flex-col" data-sveltekit-preload-data="false">
		<a class="sr-only focus:not-sr-only focus:p-4" href="#main"
			>Skip to content</a
		>
		<aside
			aria-label="Test site notice"
			class="bg-primary text-primary-foreground"
		>
			<div
				class="mx-auto flex max-w-screen-2xl items-start gap-3 px-4 py-3 text-sm md:px-6"
			>
				<TriangleAlertIcon class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
				<p>
					This is a test site; data and features may change. Consider all
					uploads may be lost and deleted at anytime.
				</p>
			</div>
		</aside>
		<header class="dark border-b bg-background text-foreground">
			<div
				class="relative mx-auto flex max-w-screen-2xl flex-wrap items-center p-4 md:px-6"
			>
				<a class="text-xl" href="/">
					<Wordmark />
				</a>
				<div class="ml-6">
					<GlobalNavigation eras={data.eras} />
				</div>
				<div class="ml-auto flex items-center gap-4">
					<Button
						variant="outline"
						size="icon"
						onclick={toggleMode}
						aria-label="Toggle theme"
					>
						{#if mode.current === 'dark'}
							<MoonIcon class="size-4" aria-hidden="true" />
						{:else}
							<SunIcon class="size-4" aria-hidden="true" />
						{/if}
						<span class="sr-only">Toggle theme</span>
					</Button>
					<MobileNavigation eras={data.eras} {session} />
					{#if session.signedIn}
						<div class="hidden sm:block">
							<DropdownMenu.Root>
								<DropdownMenu.Trigger>
									{#snippet child({ props })}
										<Button
											{...props}
											variant="outline"
											size="icon"
											class="!cursor-pointer sm:h-8 sm:w-auto sm:gap-1.5 sm:px-2.5"
											disabled={session.pending}
											aria-label={`Account: ${session.displayName}`}
										>
											<UserRoundIcon
												class="size-4 sm:hidden"
												aria-hidden="true"
											/>
											<Flag
												class="hidden sm:inline-flex"
												code={session.player?.country_code}
											/>
											<span class="hidden max-w-40 truncate sm:inline"
												>{session.pending
													? 'Signing out...'
													: session.displayName}</span
											>
											<ChevronDownIcon
												class="hidden size-4 sm:inline"
												aria-hidden="true"
											/>
										</Button>
									{/snippet}
								</DropdownMenu.Trigger>
								<DropdownMenu.Content align="end" class="dark w-48">
									<DropdownMenu.Item>
										{#snippet child({ props })}<a
												{...props}
												href="/account/hotlaps">My hotlaps</a
											>{/snippet}
									</DropdownMenu.Item>
									<DropdownMenu.Item>
										{#snippet child({ props })}<a
												{...props}
												href="/account/personal">Personal settings</a
											>{/snippet}
									</DropdownMenu.Item>
									<DropdownMenu.Item>
										{#snippet child({ props })}<a
												{...props}
												href="/account/tokens">Personal access tokens</a
											>
											<a {...props} href="/account/webhooks">Webhooks</a
											>{/snippet}
									</DropdownMenu.Item>
									<DropdownMenu.Separator />
									<DropdownMenu.Item
										disabled={session.pending}
										onSelect={session.signOut}>Sign out</DropdownMenu.Item
									>
								</DropdownMenu.Content>
							</DropdownMenu.Root>
						</div>
					{:else}
						<Button class="hidden sm:inline-flex" onclick={session.signIn}
							>Sign in with LFS</Button
						>
					{/if}
				</div>
			</div>
		</header>
		{#if session.error}
			<p
				role="alert"
				class="mx-auto w-full max-w-screen-2xl p-4 text-destructive"
			>
				{session.error}
			</p>
		{/if}
		{@render children()}
		<footer class="border-border/50 border-t">
			<div
				class="mx-auto grid max-w-screen-2xl gap-4 p-4 text-sm text-muted-foreground md:grid-cols-[1fr_auto_1fr] md:items-center md:px-6 md:py-6"
			>
				<span
					class="order-3 text-center md:order-1 md:justify-self-start md:text-left"
				>
					Independent community project · Not affiliated with
					<a
						class="hover:text-foreground hover:underline"
						href="https://www.lfs.net/">Live for Speed</a
					>
				</span>
				<nav
					aria-label="Project"
					class="order-2 flex flex-wrap justify-center gap-x-4 gap-y-2 md:order-2"
				>
					<a
						class="hover:text-foreground hover:underline"
						href="/api/docs"
						data-sveltekit-reload>API docs</a
					>
					<a
						class="hover:text-foreground hover:underline"
						href="https://github.com/theangryangel/lfspla.net/blob/main/CHANGELOG.md"
						>Changelog</a
					>
					<a
						class="hover:text-foreground hover:underline"
						href="https://www.lfs.net/forum/565-LFS-Planet-Forum">Forum</a
					>
					<a
						class="hover:text-foreground hover:underline"
						href="https://github.com/theangryangel/lfspla.net#contributing"
						>Get involved</a
					>
				</nav>
				<a
					class="order-1 justify-self-center font-medium text-foreground md:order-3 md:justify-self-end"
					href="/"
					aria-label="lfspla.net home"
				>
					<Wordmark />
				</a>
			</div>
		</footer>
		<CompareTray />
		<FloatingPanelStack />
	</div>
</Tooltip.Provider>
