<script lang="ts">
	import * as Command from '#lib/components/ui/command/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { FilterOption } from '#lib/filter-bar.js';

	let {
		label,
		value,
		placeholder,
		disabled,
		loadOptions,
		onSelect,
	}: {
		label: string;
		value: string;
		placeholder?: string;
		disabled: boolean;
		loadOptions: (
			query: string,
			signal: AbortSignal,
		) => Promise<FilterOption[]>;
		onSelect: (value: string) => Promise<unknown>;
	} = $props();
	let search = $state('');
	let options = $state<FilterOption[]>([]);
	let pending = $state(true);
	let failed = $state(false);
	let retry = $state(0);

	$effect(() => {
		const query = search.trim();
		const load = loadOptions;
		void retry;
		const controller = new AbortController();
		options = [];
		pending = true;
		failed = false;
		const timer = setTimeout(async () => {
			try {
				const results = await load(query, controller.signal);
				if (!controller.signal.aborted) options = results;
			} catch {
				if (!controller.signal.aborted) failed = true;
			} finally {
				if (!controller.signal.aborted) pending = false;
			}
		}, 200);
		return () => {
			clearTimeout(timer);
			controller.abort();
		};
	});
</script>

<h3 class="px-2.5 pt-3 pb-1 font-medium">{label}</h3>
<Command.Root shouldFilter={false} class="p-0">
	<Command.Input
		bind:value={search}
		aria-label={`Search ${label} values`}
		placeholder={placeholder ?? `Search ${label.toLowerCase()}...`}
	/>
	<Command.List aria-busy={pending} class="p-1">
		<Command.Item value="__any__" {disabled} onSelect={() => void onSelect('')}
			>Any</Command.Item
		>
		{#if pending}
			<p role="status" class="p-3 text-sm text-muted-foreground">
				Searching...
			</p>
		{:else if failed}
			<div class="space-y-2 p-3">
				<p role="alert" class="text-sm text-destructive">
					Could not load suggestions.
				</p>
				<Button variant="outline" size="sm" onclick={() => retry++}
					>Retry</Button
				>
			</div>
		{:else if options.length === 0}
			<p role="status" class="p-3 text-sm text-muted-foreground">
				No matching values.
			</p>
		{:else}
			{#each options as option (option.value)}
				<Command.Item
					value={option.value}
					{disabled}
					data-checked={value === option.value ? 'true' : undefined}
					onSelect={() => void onSelect(option.value)}
				>
					<span class="min-w-0">
						<span class="block truncate">{option.label}</span>
						{#if option.description}<span
								class="block truncate text-sm text-muted-foreground"
								>{option.description}</span
							>{/if}
					</span>
				</Command.Item>
			{/each}
		{/if}
	</Command.List>
</Command.Root>
