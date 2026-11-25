<script lang="ts">
	import CircleHelp from '@lucide/svelte/icons/circle-help';
	import { Button } from '#lib/components/ui/button/index.js';
	import { useHelp } from '#lib/help.svelte.js';

	const help = useHelp();
</script>

<Button
	data-help-trigger
	variant="ghost"
	aria-expanded={help.open}
	aria-controls="page-help"
	onclick={(event) => help.toggle(event.currentTarget)}
>
	<CircleHelp aria-hidden="true" />Help
</Button>
