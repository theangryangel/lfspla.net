<script lang="ts">
	import type { LayoutProps } from './$types';
	import { page } from '$app/state';
	import Breadcrumbs from '$lib/components/app/Breadcrumbs.svelte';
	import PageHeading from '$lib/components/app/PageHeading.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { useSession } from '$lib/session.svelte.js';
	let { children }: LayoutProps = $props();
	const session = useSession();
</script>

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<Breadcrumbs items={page.data.breadcrumbs} />
	{#if session.signedIn}
		{@render children()}
	{:else}
		<PageHeading title="Your account"
			>Sign in to manage your personal settings and access tokens.</PageHeading
		>
		<Button onclick={session.signIn}>Sign in with LFS</Button>
	{/if}
</main>
