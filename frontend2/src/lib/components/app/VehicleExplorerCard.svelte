<script lang="ts">
	import StarIcon from '@lucide/svelte/icons/star';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import type { ExplorerVehicle } from '#lib/api.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import {
		drives,
		transmissions,
		vehicleClasses,
	} from '#lib/vehicle-explorer.js';
	import Thumbnail from './Thumbnail.svelte';

	let { vehicle }: { vehicle: ExplorerVehicle } = $props();
	const mod = $derived(/^[0-9A-F]{6}$/i.test(vehicle.code));
	const specs = $derived(vehicle.specifications);
	const number = (value: number) => Math.round(value).toLocaleString();
	const details = $derived([
		['Class', vehicleClasses[vehicle.class ?? -1]],
		[
			'Engine',
			vehicle.electric
				? 'Electric'
				: specs?.displacement_cc != null
					? `${(specs.displacement_cc / 1000).toFixed(1)} L`
					: vehicle.electric === false
						? 'Combustion'
						: null,
		],
		['Drivetrain', drives[specs?.drive ?? -1]],
		['Gearbox', transmissions[specs?.transmission ?? -1]],
		...(specs
			? [
					[
						'Power',
						specs.power_kw != null ? `${number(specs.power_kw)} kW` : '—',
					],
					[
						'Torque',
						specs.torque_nm != null ? `${number(specs.torque_nm)} Nm` : '—',
					],
					['Mass', specs.mass_kg != null ? `${number(specs.mass_kg)} kg` : '—'],
					[
						'Power / weight',
						specs.power_weight_w_kg != null
							? `${number(specs.power_weight_w_kg)} W/kg`
							: '—',
					],
				]
			: []),
	]);
</script>

<article
	class="flex h-full flex-col overflow-hidden rounded-sm border bg-card text-card-foreground"
>
	<Thumbnail src={vehicle.image?.url} />
	<div class="flex flex-1 flex-col gap-3 p-3 text-sm">
		<div>
			<h2 class="break-words text-sm font-medium">
				{#if mod}<a
						class="hover:underline"
						href="https://www.lfs.net/files/vehmods/{vehicle.code}"
						>{vehicle.name}</a
					>{:else}{vehicle.name}{/if}
			</h2>
			<p class="mt-2 text-muted-foreground">
				<span class="font-mono">{vehicle.code}</span>
			</p>
			{#if vehicle.author}<p class="mt-1 break-words text-muted-foreground">
					By <a
						href="/vehicles?author={encodeURIComponent(vehicle.author)}"
						class="hover:underline">{vehicle.author}</a
					>
				</p>{/if}
		</div>
		<div class="flex flex-wrap gap-1.5">
			<Badge variant="secondary">{mod ? 'Mod' : 'Standard'}</Badge>
			<Badge variant="secondary">{vehicle.license.toUpperCase()}</Badge>
			{#if vehicle.approved}<Badge variant="secondary">Approved</Badge>{/if}
		</div>
		{#if details.some(([, value]) => value != null)}
			<dl class="-mx-3 grid grid-cols-2 gap-x-3 gap-y-2 border-t px-3 py-2">
				{#each details as [label, value]}
					{#if value != null}<div class="min-w-0">
							<dt class="text-muted-foreground">{label}</dt>
							<dd class="mt-0.5 break-words font-medium tabular-nums">
								{value}
							</dd>
						</div>{/if}
				{/each}
			</dl>
		{/if}
		{#if !specs}<p class="text-muted-foreground">
				Specifications unavailable
			</p>{/if}
		{#if vehicle.rating != null || vehicle.downloads != null}
			<div
				class="-mx-3 mt-auto flex flex-wrap justify-between gap-x-3 gap-y-1 border-t px-3 pt-3 text-muted-foreground"
			>
				{#if vehicle.rating != null}
					<span class="inline-flex items-center gap-1.5">
						<StarIcon class="size-3.5 shrink-0" aria-hidden="true" />
						<span class="sr-only">Rating</span>
						{vehicle.rating.toFixed(2)} ({vehicle.num_ratings ?? 0} ratings)
					</span>
				{/if}
				{#if vehicle.downloads != null}
					<span class="inline-flex items-center gap-1.5">
						<DownloadIcon class="size-3.5 shrink-0" aria-hidden="true" />
						{number(vehicle.downloads)} downloads
					</span>
				{/if}
			</div>
		{/if}
	</div>
</article>
