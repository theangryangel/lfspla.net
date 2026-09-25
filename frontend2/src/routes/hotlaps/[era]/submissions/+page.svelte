<script lang="ts">
	import HotlapSubmissions from '$lib/components/app/HotlapSubmissions.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<HotlapSubmissions hotlaps={data.hotlaps} era={data.era} />
