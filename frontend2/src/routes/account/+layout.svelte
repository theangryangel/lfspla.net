<script lang="ts">
	import TimerIcon from '@lucide/svelte/icons/timer';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';
	import WebhookIcon from '@lucide/svelte/icons/webhook';
	import type { LayoutProps } from './$types';
	import { page } from '$app/state';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import PageNavigation from '#lib/components/app/PageNavigation.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { useSession } from '#lib/session.svelte.js';
	let { children }: LayoutProps = $props();
	const session = useSession();
	const sections = [
		{
			label: 'My hotlaps',
			icon: TimerIcon,
			to: '/account/hotlaps',
			description: 'All your hotlap submissions across every era.',
		},
		{
			label: 'Personal settings',
			icon: SettingsIcon,
			to: '/account/personal',
			description: 'Manage the preferences for your LFS account.',
		},
		{
			label: 'Personal access tokens',
			icon: KeyRoundIcon,
			to: '/account/tokens',
			description:
				'Create credentials for tools that use the API on your behalf. Keep them private.',
		},
		{
			label: 'Webhooks',
			icon: WebhookIcon,
			to: '/account/webhooks',
			description:
				'Choose which notifications to receive and where to send them.',
		},
	];
	const current = $derived(
		sections.find((section) => section.to === page.url.pathname),
	);
</script>

<main
	id="main"
	class="mx-auto grid w-full min-w-0 max-w-screen-2xl flex-1 content-start gap-6 p-4 md:grid-cols-[14rem_minmax(0,1fr)] md:gap-8 md:p-6"
>
	<aside
		class="min-w-0 space-y-3 border-b pb-4 md:border-b-0 md:border-r md:pr-6"
		aria-label="Account options"
	>
		<h2 class="racing-section-title">My account</h2>
		<PageNavigation label="Account" items={sections} orientation="vertical" />
	</aside>
	<div class="min-w-0 space-y-6">
		<header class="border-b pb-4">
			<PageHeading title={current?.label ?? 'Your account'}
				>{current?.description}</PageHeading
			>
		</header>
		{#if session.signedIn}
			{@render children()}
		{:else}
			<p class="text-sm text-muted-foreground">
				Sign in to manage your account.
			</p>
			<Button onclick={session.signIn}>Sign in with LFS</Button>
		{/if}
	</div>
</main>
