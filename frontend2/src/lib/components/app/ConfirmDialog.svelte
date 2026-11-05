<script lang="ts">
	import { AlertDialog } from 'bits-ui';
	import { tick } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		open = $bindable(false),
		title,
		description,
		confirmLabel,
		pendingLabel = 'Removing…',
		busy = false,
		error = '',
		onConfirm,
	}: {
		open?: boolean;
		title: string;
		description: string;
		confirmLabel: string;
		pendingLabel?: string;
		busy?: boolean;
		error?: string;
		onConfirm: () => void | Promise<void>;
	} = $props();
	let cancelButton = $state<HTMLButtonElement | null>(null);
</script>

<AlertDialog.Root
	{open}
	onOpenChange={(value) => {
		if (!busy) open = value;
	}}
>
	<AlertDialog.Portal>
		<AlertDialog.Overlay
			class="fixed inset-0 z-50 isolate bg-black/10 duration-100 supports-backdrop-filter:backdrop-blur-xs data-open:animate-in data-open:fade-in-0 data-closed:animate-out data-closed:fade-out-0"
		/>
		<AlertDialog.Content
			onOpenAutoFocus={(event) => {
				event.preventDefault();
				void tick().then(() => cancelButton?.focus());
			}}
			onEscapeKeydown={(event) => {
				if (busy) event.preventDefault();
			}}
			aria-busy={busy}
			class="fixed top-1/2 left-1/2 z-50 grid w-full max-w-[calc(100%_-_2rem)] -translate-x-1/2 -translate-y-1/2 gap-4 rounded-xl bg-popover p-4 text-sm text-popover-foreground ring-1 ring-foreground/10 outline-none duration-100 sm:max-w-sm data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95"
		>
			<div class="flex flex-col gap-2">
				<AlertDialog.Title class="racing-section-title"
					>{title}</AlertDialog.Title
				>
				<AlertDialog.Description class="text-sm text-muted-foreground"
					>{description}</AlertDialog.Description
				>
			</div>
			{#if error}<p role="alert" class="text-sm text-destructive">
					{error}
				</p>{/if}
			<div
				class="-mx-4 -mb-4 flex flex-col-reverse gap-2 rounded-b-xl border-t bg-accent/50 p-4 sm:flex-row sm:justify-end"
			>
				<AlertDialog.Cancel bind:ref={cancelButton} disabled={busy}>
					{#snippet child({ props })}<Button {...props} variant="outline"
							>Cancel</Button
						>{/snippet}
				</AlertDialog.Cancel>
				<AlertDialog.Action
					disabled={busy}
					onclick={() => {
						if (!busy) void onConfirm();
					}}
				>
					{#snippet child({ props })}<Button {...props} variant="destructive"
							>{busy ? pendingLabel : confirmLabel}</Button
						>{/snippet}
				</AlertDialog.Action>
			</div>
		</AlertDialog.Content>
	</AlertDialog.Portal>
</AlertDialog.Root>
