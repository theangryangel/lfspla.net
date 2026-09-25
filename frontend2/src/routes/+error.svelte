<script lang="ts">
	import { page } from '$app/state';
	import Empty from '$lib/components/app/Empty.svelte';
</script>

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<Empty
		title={page.status === 404 ? 'Page not found' : 'Something went wrong'}
	>
		{#if page.status !== 404}
			<p>{page.error?.message}</p>
		{/if}
		<a href="/">Back home</a>
	</Empty>
</main>
