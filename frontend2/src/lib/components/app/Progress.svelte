<script lang="ts">
	import { Progress as ProgressBar } from '$lib/components/ui/progress/index.js';

	let { value, total }: { value: number; total: number } = $props();
</script>

<div class="min-w-40 space-y-2">
	<div class="flex justify-between gap-4 text-sm">
		<span>{value.toLocaleString()} / {total.toLocaleString()}</span>
		<span class="text-muted-foreground">
			{Math.round((value / (total || 1)) * 100)}%
		</span>
	</div>
	<ProgressBar {value} max={total || 1} aria-label="Completed combinations" />
</div>
