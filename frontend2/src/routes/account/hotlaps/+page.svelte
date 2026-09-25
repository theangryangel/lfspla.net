<script lang="ts">
	import HotlapSubmissions from '$lib/components/app/HotlapSubmissions.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<svelte:head><title>My hotlaps · lfspla.net</title></svelte:head>
<p class="text-sm text-muted-foreground">
	All your hotlap submissions across every era.
</p>
<HotlapSubmissions hotlaps={data.hotlaps} />
