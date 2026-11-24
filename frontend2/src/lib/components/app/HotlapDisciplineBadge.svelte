<script lang="ts">
	import type { HotLapDiscipline } from '#lib/api.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	let { discipline }: { discipline: HotLapDiscipline } = $props();
</script>

<Badge
	variant="outline"
	class={discipline === 'drag'
		? 'border-purple-400/40 bg-purple-400/10 text-purple-700 dark:text-purple-300'
		: 'border-yellow-500/40 bg-brand-yellow/10 text-yellow-800 dark:text-brand-yellow'}
>
	{discipline === 'drag' ? 'Drag' : 'Hotlap'}
</Badge>
