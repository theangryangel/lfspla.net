<script lang="ts">
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { disciplineLabels } from '#lib/teams.js';
	import type { Tag } from '#lib/api.js';
	let {
		value = $bindable<Tag[]>([]),
		disabled = false,
	}: { value?: Tag[]; disabled?: boolean } = $props();
	const id = $props.id();
	function select(tag: Tag, checked: boolean) {
		value = checked
			? [...value.filter((item) => item !== tag), tag]
			: value.filter((item) => item !== tag);
	}
</script>

<div
	class="flex flex-wrap items-center gap-x-6 gap-y-3"
	role="group"
	aria-label="Team disciplines"
>
	{#each Object.entries(disciplineLabels) as [tag, label]}
		<div class="flex items-center gap-2">
			<Checkbox
				id={`${id}-${tag}`}
				checked={value.includes(tag as Tag)}
				onCheckedChange={(checked) => select(tag as Tag, checked)}
				{disabled}
			/>
			<label for={`${id}-${tag}`} class="cursor-pointer text-sm font-medium"
				>{label}</label
			>
		</div>
	{/each}
</div>
