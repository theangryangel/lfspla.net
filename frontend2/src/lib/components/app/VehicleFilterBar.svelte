<script lang="ts">
	import { tick } from 'svelte';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import XIcon from '@lucide/svelte/icons/x';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import ChoiceSelect from './ChoiceSelect.svelte';
	import VehicleSpecRange from './VehicleSpecRange.svelte';
	import { selects, ranges, sorts, filterKeys } from '#lib/vehicle-explorer.js';
	import { setQuery } from '#lib/query.js';

	let {
		filters,
		loading,
	}: { filters: Record<string, string>; loading: boolean } = $props();
	type Definition = {
		key: string;
		label: string;
		options?: { value: string; label: string }[];
		range?: (typeof ranges)[number];
	};
	const definitions: Definition[] = [
		{ key: 'q', label: 'Name or code' },
		...selects.map((select) => ({
			...select,
			options: select.options.map(([value, label]) => ({ value, label })),
		})),
		...ranges.map((range) => ({ key: range.key, label: range.label, range })),
		{ key: 'author', label: 'Uploaded by' },
	];
	let addOpen = $state(false);
	let editor = $state<string | null>(null);
	let errorMessage = $state('');
	let addButton = $state<HTMLButtonElement | null>(null);
	const active = (definition: Definition) =>
		definition.range
			? Boolean(
					filters[`min_${definition.key}`] || filters[`max_${definition.key}`],
				)
			: Boolean(filters[definition.key]);
	const shown = $derived(
		definitions.filter(
			(definition) => active(definition) || editor === definition.key,
		),
	);
	const available = $derived(
		definitions.filter(
			(definition) => !active(definition) && editor !== definition.key,
		),
	);
	const hasFilters = $derived(definitions.some(active));
	const number = (value: string) =>
		Number(value).toLocaleString(undefined, { maximumFractionDigits: 20 });

	function summary(definition: Definition) {
		if (definition.range) {
			const min = filters[`min_${definition.key}`];
			const max = filters[`max_${definition.key}`];
			if (min && max)
				return `${number(min)}–${number(max)} ${definition.range.unit}`;
			if (min) return `≥ ${number(min)} ${definition.range.unit}`;
			if (max) return `≤ ${number(max)} ${definition.range.unit}`;
			return 'Any';
		}
		return (
			definition.options?.find(
				(option) => option.value === filters[definition.key],
			)?.label ||
			filters[definition.key] ||
			'Any'
		);
	}

	async function apply(values: Record<string, string>) {
		errorMessage = '';
		try {
			await setQuery({ ...values, page: '' });
			return true;
		} catch {
			errorMessage = 'Could not update the vehicle filters. Please try again.';
			return false;
		}
	}
	async function addFilter(key: string) {
		addOpen = false;
		await tick();
		errorMessage = '';
		editor = key;
	}
	async function choose(definition: Definition, value: string) {
		if (await apply({ [definition.key]: value })) editor = null;
	}
	async function applyRange(definition: Definition, min: string, max: string) {
		if (
			[min, max].some(
				(value) =>
					value && (!Number.isFinite(Number(value)) || Number(value) < 0),
			)
		) {
			errorMessage = 'Use non-negative numbers for specification ranges.';
			return false;
		}
		if (min && max && Number(min) > Number(max)) {
			errorMessage = 'Minimum must not exceed maximum.';
			return false;
		}
		return apply({
			[`min_${definition.key}`]: min,
			[`max_${definition.key}`]: max,
		});
	}
	async function remove(definition: Definition) {
		const wasActive = active(definition);
		editor = null;
		if (wasActive)
			await apply(
				definition.range
					? { [`min_${definition.key}`]: '', [`max_${definition.key}`]: '' }
					: { [definition.key]: '' },
			);
	}
	async function submitRange(event: SubmitEvent, definition: Definition) {
		event.preventDefault();
		const form = new FormData(event.currentTarget as HTMLFormElement);
		const min = String(form.get(`min_${definition.key}`) ?? '').trim();
		const max = String(form.get(`max_${definition.key}`) ?? '').trim();
		if (await applyRange(definition, min, max)) editor = null;
	}
</script>

<div class="flex flex-wrap items-start gap-3 border-b pb-4">
	<div
		class="flex min-h-8 min-w-0 flex-1 basis-[28rem] flex-wrap items-center gap-2"
		role="group"
		aria-label="Vehicle filters"
	>
		{#each shown as definition (definition.key)}
			<Popover.Root
				open={editor === definition.key}
				onOpenChange={(open) => {
					editor = open ? definition.key : null;
					errorMessage = '';
				}}
			>
				<div
					class="inline-flex max-w-full items-center rounded-sm border border-input bg-background"
				>
					<Popover.Trigger>
						{#snippet child({ props })}
							<Button
								{...props}
								variant="ghost"
								class="h-8 min-w-0 gap-1.5 rounded-r-none border-0 px-2 font-normal"
								aria-label={`Edit ${definition.label} filter`}
								title={`Current value: ${summary(definition)}`}
							>
								<span class="truncate text-muted-foreground"
									>{definition.label}:</span
								>
								<span class="truncate font-medium">{summary(definition)}</span>
								<ChevronDownIcon
									class="size-3 shrink-0 text-muted-foreground"
									aria-hidden="true"
								/>
							</Button>
						{/snippet}
					</Popover.Trigger>
					<Button
						type="button"
						variant="ghost"
						class="h-8 w-7 shrink-0 rounded-l-none border-0 px-0 text-muted-foreground"
						aria-label={`Remove ${definition.label} filter`}
						disabled={loading}
						onclick={() => remove(definition)}
						><XIcon class="size-3" aria-hidden="true" /></Button
					>
				</div>
				<Popover.Content
					align="start"
					class="w-72 max-w-[calc(100vw-2rem)] p-3"
					aria-label={`${definition.label} filter`}
					onCloseAutoFocus={(event) => {
						if (!active(definition)) {
							event.preventDefault();
							addButton?.focus();
						}
					}}
				>
					{#if definition.options}
						<h3 class="font-medium">{definition.label}</h3>
						<Command.Root>
							<Command.Input
								aria-label={`Search ${definition.label} values`}
								placeholder={`Search ${definition.label.toLowerCase()}…`}
							/>
							<Command.List>
								<Command.Empty>No matching values.</Command.Empty>
								{#each [{ value: '', label: 'Any' }, ...definition.options] as option (option.value)}
									<Command.Item
										value={option.value || '__any__'}
										keywords={[option.label]}
										data-checked={filters[definition.key] === option.value
											? 'true'
											: undefined}
										disabled={loading}
										onSelect={() => choose(definition, option.value)}
										>{option.label}</Command.Item
									>
								{/each}
							</Command.List>
						</Command.Root>
					{:else if definition.range}
						<form
							onsubmit={(event) => submitRange(event, definition)}
							class="space-y-3"
						>
							<VehicleSpecRange
								name={definition.key}
								label={`${definition.label} (${definition.range.unit})`}
								ceiling={definition.range.slider.ceiling}
								step={definition.range.slider.step}
								minimum={filters[`min_${definition.key}`]}
								maximum={filters[`max_${definition.key}`]}
								onCommit={(min, max) => {
									void applyRange(definition, min, max);
								}}
							/>
							<p class="text-xs text-muted-foreground">
								The + end leaves the maximum open.
							</p>
							<Button
								type="button"
								variant="outline"
								class="w-full"
								onclick={() => (editor = null)}>Done</Button
							>
						</form>
					{:else}
						<form
							class="space-y-3"
							onsubmit={async (event) => {
								event.preventDefault();
								const form = new FormData(
									event.currentTarget as HTMLFormElement,
								);
								await choose(
									definition,
									String(form.get(definition.key) ?? '').trim(),
								);
							}}
						>
							<h3 class="font-medium">{definition.label}</h3>
							<Input
								name={definition.key}
								value={filters[definition.key]}
								aria-label={definition.label}
								placeholder={definition.key === 'q'
									? 'Name or code…'
									: 'LFS username'}
							/>
							<Button type="submit" class="w-full" disabled={loading}
								>Apply filter</Button
							>
						</form>
					{/if}
					{#if errorMessage}<p role="alert" class="text-sm text-destructive">
							{errorMessage}
						</p>{/if}
				</Popover.Content>
			</Popover.Root>
		{/each}
		<Popover.Root bind:open={addOpen}>
			<Popover.Trigger>
				{#snippet child({ props })}
					<Button
						{...props}
						bind:ref={addButton}
						variant="outline"
						class="border-dashed"
						disabled={available.length === 0 || loading}
						><PlusIcon class="size-4" aria-hidden="true" />Add filter</Button
					>
				{/snippet}
			</Popover.Trigger>
			<Popover.Content
				align="start"
				class="w-64 p-0"
				aria-label="Add a vehicle filter"
				onCloseAutoFocus={(event) => {
					if (editor) event.preventDefault();
				}}
			>
				<Command.Root>
					<Command.Input
						aria-label="Find a filter"
						placeholder="Select a filter…"
					/>
					<Command.List>
						<Command.Empty>No matching filters.</Command.Empty>
						{#each available as definition (definition.key)}<Command.Item
								value={definition.key}
								keywords={[definition.label]}
								onSelect={() => addFilter(definition.key)}
								>{definition.label}</Command.Item
							>{/each}
					</Command.List>
				</Command.Root>
			</Popover.Content>
		</Popover.Root>
		{#if hasFilters}<Button
				variant="ghost"
				disabled={loading}
				onclick={() => {
					editor = null;
					void apply(
						Object.fromEntries(
							filterKeys
								.filter((key) => key !== 'sort')
								.map((key) => [key, '']),
						),
					);
				}}>Clear filters</Button
			>{/if}
	</div>
	<div class="ml-auto shrink-0">
		<ChoiceSelect
			label="Sort vehicles"
			value={filters.sort || 'catalogue'}
			options={sorts.map(([value, label]) => ({ value, label }))}
			disabled={loading}
			class="min-w-48"
			onValueChange={(value) => apply({ sort: value })}
		/>
	</div>
	{#if errorMessage && !editor}<p role="alert" class="text-sm text-destructive">
			{errorMessage}
		</p>{/if}
</div>
