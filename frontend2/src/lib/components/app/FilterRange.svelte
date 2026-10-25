<script lang="ts">
	import { Slider } from 'bits-ui';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Button } from '#lib/components/ui/button/index.js';

	let {
		name,
		label,
		ceiling,
		step,
		minimum = '',
		maximum = '',
		onCommit,
	}: {
		name: string;
		label: string;
		ceiling: number;
		step: number;
		minimum?: string;
		maximum?: string;
		onCommit: (minimum: string, maximum: string) => void;
	} = $props();

	let lower = $derived(minimum);
	let upper = $derived(maximum);
	const numeric = (value: string) =>
		value !== '' && Number.isFinite(Number(value)) ? Number(value) : undefined;
	// Anchor the scale to applied URL values so it cannot jump during a drag.
	// Explicit values beyond the usual scale expand it instead of being discarded.
	const limit = $derived(
		Math.max(
			ceiling,
			Math.ceil(
				(Math.max(numeric(minimum) ?? 0, numeric(maximum) ?? 0) + step) /
					ceiling,
			) * ceiling,
		),
	);
	const values = $derived.by(() => {
		const min = Math.max(0, Math.min(limit, numeric(lower) ?? 0));
		const max = Math.max(0, Math.min(limit, numeric(upper) ?? limit));
		// Keep slider positions on its coarse steps without rewriting precise inputs.
		return [Math.min(min, max), Math.max(min, max)].map(
			(value) => Math.round(value / step) * step,
		);
	});

	function slide(next: number[]) {
		if (next[0] !== values[0]) lower = next[0] <= 0 ? '' : String(next[0]);
		if (next[1] !== values[1]) upper = next[1] >= limit ? '' : String(next[1]);
	}
	function reset() {
		lower = '';
		upper = '';
		onCommit(lower, upper);
	}
</script>

<fieldset class="min-w-0 space-y-2">
	<legend class="sr-only">{label}</legend>
	<div class="flex h-5 items-center justify-between gap-2">
		<span class="text-sm font-medium">{label}</span>
		{#if lower || upper}<Button
				type="button"
				variant="ghost"
				size="xs"
				class="h-5 px-1 text-xs text-muted-foreground"
				aria-label={`Reset ${label} range`}
				onclick={reset}>Reset</Button
			>{/if}
	</div>
	<div class="px-2">
		<Slider.Root
			type="multiple"
			min={0}
			max={limit}
			{step}
			autoSort={false}
			value={values}
			onValueChange={slide}
			onValueCommit={() => onCommit(lower, upper)}
			aria-label={`${label} range`}
			data-spec-range={name}
			class="relative flex h-7 w-full touch-none select-none items-center"
		>
			{#snippet children({ thumbItems })}
				<span
					class="relative h-1 w-full overflow-hidden rounded-full bg-border"
				>
					<Slider.Range class="absolute h-full bg-primary" />
				</span>
				{#each thumbItems as thumb (thumb.index)}
					<Slider.Thumb
						index={thumb.index}
						aria-label={`${thumb.index === 0 ? 'Minimum' : 'Maximum'} ${label}`}
						aria-valuetext={thumb.index === 0 && !lower
							? 'No minimum'
							: thumb.index === 1 && !upper
								? 'No maximum'
								: String(thumb.value)}
						class="block size-4 rounded-full border-2 border-primary bg-background shadow-sm outline-none transition-shadow hover:ring-3 hover:ring-primary/20 focus-visible:ring-3 focus-visible:ring-ring/50"
					/>
				{/each}
			{/snippet}
		</Slider.Root>
		<div
			class="flex justify-between text-[0.65rem] tabular-nums text-muted-foreground"
			aria-hidden="true"
		>
			<span>0</span><span>{limit.toLocaleString()}+</span>
		</div>
	</div>
	<div class="flex items-center gap-2">
		<Input
			type="number"
			min="0"
			step="any"
			name={`min_${name}`}
			value={lower}
			oninput={(event) => (lower = event.currentTarget.value)}
			onchange={() => onCommit(lower, upper)}
			aria-label={`Minimum ${label}`}
			placeholder="Min"
			class="min-w-0"
		/>
		<span class="text-xs text-muted-foreground" aria-hidden="true">–</span>
		<Input
			type="number"
			min="0"
			step="any"
			name={`max_${name}`}
			value={upper}
			oninput={(event) => (upper = event.currentTarget.value)}
			onchange={() => onCommit(lower, upper)}
			aria-label={`Maximum ${label}`}
			placeholder="Any"
			class="min-w-0"
		/>
	</div>
</fieldset>
