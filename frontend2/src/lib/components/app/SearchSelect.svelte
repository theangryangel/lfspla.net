<script lang="ts" module>
	export interface SearchOption {
		value: string;
		label: string;
		hint?: string;
	}
</script>

<script lang="ts">
	import ChevronsUpDownIcon from '@lucide/svelte/icons/chevrons-up-down';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Command from '$lib/components/ui/command/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';

	let {
		label,
		value,
		selectedLabel = '',
		options,
		search,
		placeholder = 'Search...',
		onValueChange,
	}: {
		label: string;
		value: string;
		selectedLabel?: string;
		options: SearchOption[];
		search?: (term: string) => Promise<SearchOption[]>;
		placeholder?: string;
		onValueChange: (value: string) => void;
	} = $props();

	// bits-ui reserves the empty string for no selection.
	const ALL = '__all__';
	const encode = (v: string) => v || ALL;

	let open = $state(false);
	let term = $state('');
	let results = $state<SearchOption[]>([]);
	let pending = $state(false);
	let failed = $state(false);

	// Ignore stale search responses.
	let latest = 0;
	let timer: ReturnType<typeof setTimeout> | undefined;

	const items = $derived(search && term ? results : options);
	const current = $derived(
		selectedLabel ||
			options.find((option) => option.value === value)?.label ||
			'',
	);

	function onSearch(next: string) {
		if (!search) return;

		clearTimeout(timer);
		const token = ++latest;
		if (!next) {
			pending = false;
			failed = false;
			return;
		}
		failed = false;
		results = [];
		pending = true;
		timer = setTimeout(async () => {
			try {
				const found = await search(next);
				if (token !== latest) return;
				results = found;
				failed = false;
			} catch {
				if (token !== latest) return;
				failed = true;
			} finally {
				if (token === latest) pending = false;
			}
		}, 200);
	}

	function choose(next: string) {
		open = false;
		term = '';
		onSearch('');
		if (next !== value) onValueChange(next);
	}

	$effect(() => () => clearTimeout(timer));
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="outline"
				role="combobox"
				aria-label={label}
				aria-expanded={open}
				class="w-56 justify-between font-normal"
			>
				<span class="truncate">{current || label}</span>
				<ChevronsUpDownIcon class="opacity-50" />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-56 p-0">
		<Command.Root shouldFilter={!search}>
			<Command.Input
				{placeholder}
				bind:value={term}
				oninput={(e) => onSearch(e.currentTarget.value)}
			/>
			<Command.List>
				{#if pending}
					<Command.Loading>
						<p class="py-6 text-center text-sm text-muted-foreground">
							Searching...
						</p>
					</Command.Loading>
				{:else if failed}
					<p role="alert" class="py-6 text-center text-sm text-destructive">
						The search could not be completed.
					</p>
				{:else}
					<Command.Empty>No matches.</Command.Empty>
					{#each items as option (option.value)}
						<Command.Item
							value={encode(option.value)}
							keywords={[option.label]}
							data-checked={option.value === value ? 'true' : undefined}
							onSelect={() => choose(option.value)}
						>
							<span class="truncate">{option.label}</span>
							{#if option.hint}
								<span class="text-xs text-muted-foreground">{option.hint}</span>
							{/if}
						</Command.Item>
					{/each}
				{/if}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
