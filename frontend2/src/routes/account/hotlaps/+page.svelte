<script lang="ts">
	import HotlapSubmissions from '$lib/components/app/HotlapSubmissions.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<svelte:head><title>My hotlaps · lfspla.net</title></svelte:head>

<HotlapSubmissions hotlaps={data.hotlaps} />
