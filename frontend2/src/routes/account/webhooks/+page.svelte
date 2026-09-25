<script lang="ts">
	import {
		send,
		type WebhookNotificationResponse,
		type WebhookResponse,
	} from '$lib/api.js';
	import { useSession } from '$lib/session.svelte.js';
	import { dateTime } from '$lib/format.js';
	import Panel from '$lib/components/app/Panel.svelte';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const session = useSession();
	const endpoint = '/api/v1/me/webhooks';
	let webhooks = $derived(data.webhooks);
	let format = $state('');
	let eventKind = $state('');
	$effect(() => {
		if (!format) format = data.options?.formats[0]?.value ?? '';
		if (!eventKind) eventKind = data.options?.events[0]?.value ?? '';
	});
	const selectedFormat = $derived(
		data.options?.formats.find((option) => option.value === format),
	);
	let name = $state('');
	let url = $state('');
	let pending = $state(false);
	let error = $state('');
	let notice = $state('');
	let target = $state<WebhookResponse | null>(null);
	let confirmOpen = $state(false);

	function eventLabel(event: WebhookNotificationResponse['event_kind']) {
		return event === 'world_record_set'
			? 'World record set'
			: 'Hotlap validated';
	}

	function statusLabel(status: WebhookNotificationResponse['status']) {
		return status[0].toUpperCase() + status.slice(1);
	}

	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !format || !eventKind) return;
		pending = true;
		error = '';
		notice = '';
		try {
			const webhook = await send<WebhookResponse>(endpoint, {
				method: 'POST',
				csrf: session.me.csrf_token,
				json: {
					name: name.trim(),
					url: url.trim(),
					format,
					event_kind: eventKind,
				},
			});
			if (!webhook)
				throw new Error(
					'The server did not return the webhook. Reload before trying again.',
				);
			webhooks = [webhook, ...webhooks];
			name = '';
			url = '';
			notice = 'Webhook added.';
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Could not add webhook.';
		} finally {
			pending = false;
		}
	}
	async function toggle(webhook: WebhookResponse) {
		if (pending) return;
		pending = true;
		error = '';
		notice = '';
		try {
			const updated = await send<WebhookResponse>(`${endpoint}/${webhook.id}`, {
				method: 'PATCH',
				csrf: session.me.csrf_token,
				json: { enabled: !webhook.enabled },
			});
			if (!updated) throw new Error('Could not update webhook.');
			webhooks = webhooks.map((item) =>
				item.id === updated.id ? updated : item,
			);
			notice = `${updated.name} ${updated.enabled ? 'resumed' : 'paused'}.`;
		} catch (cause) {
			error =
				cause instanceof Error ? cause.message : 'Could not update webhook.';
		} finally {
			pending = false;
		}
	}
	async function remove() {
		if (!target || pending) return;
		pending = true;
		error = '';
		notice = '';
		try {
			await send(`${endpoint}/${target.id}`, {
				method: 'DELETE',
				csrf: session.me.csrf_token,
			});
			webhooks = webhooks.filter((item) => item.id !== target?.id);
			notice = 'Webhook deleted.';
			confirmOpen = false;
		} catch (cause) {
			error =
				cause instanceof Error ? cause.message : 'Could not delete webhook.';
		} finally {
			pending = false;
		}
	}
</script>

<svelte:head><title>Webhooks · lfspla.net</title></svelte:head>
<p class="text-sm text-muted-foreground">
	Choose which notifications to receive and where to send them.
</p>
{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
{#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
<div class="max-w-2xl space-y-6">
	<Panel title="Add a webhook">
		<form onsubmit={create} class="space-y-4">
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-2">
					<label for="webhook-format" class="text-sm font-medium"
						>Destination type</label
					>
					<Select.Root
						type="single"
						value={format}
						onValueChange={(value) => (format = value ?? '')}
					>
						<Select.Trigger
							id="webhook-format"
							class="w-full"
							disabled={pending}
						>
							{selectedFormat?.label ?? 'Choose a destination'}
						</Select.Trigger>
						<Select.Content>
							{#each data.options?.formats ?? [] as option (option.value)}
								<Select.Item value={option.value} label={option.label} />
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-2">
					<label for="webhook-event" class="text-sm font-medium"
						>Notify me when</label
					>
					<Select.Root
						type="single"
						value={eventKind}
						onValueChange={(value) => (eventKind = value ?? '')}
					>
						<Select.Trigger
							id="webhook-event"
							class="w-full"
							disabled={pending}
						>
							{data.options?.events.find((option) => option.value === eventKind)
								?.label ?? 'Choose an event'}
						</Select.Trigger>
						<Select.Content>
							{#each data.options?.events ?? [] as option (option.value)}
								<Select.Item value={option.value} label={option.label} />
							{/each}
						</Select.Content>
					</Select.Root>
					{#if data.options?.events.find((option) => option.value === eventKind)?.description}
						<p class="text-sm text-muted-foreground">
							{data.options?.events.find((option) => option.value === eventKind)
								?.description}
						</p>
					{/if}
				</div>
			</div>
			<div class="space-y-2">
				<label for="webhook-name" class="text-sm font-medium">Name</label>
				<Input
					id="webhook-name"
					bind:value={name}
					required
					maxlength={100}
					disabled={pending}
					placeholder="e.g. Racing club"
				/>
			</div>
			<div class="space-y-2">
				<label for="webhook-url" class="text-sm font-medium"
					>{selectedFormat?.label ?? 'Destination'} webhook URL</label
				>
				<Input
					id="webhook-url"
					type="password"
					bind:value={url}
					required
					maxlength={2048}
					disabled={pending}
					autocomplete="off"
					placeholder={selectedFormat?.url_placeholder}
					aria-describedby="webhook-help"
				/>
				<p id="webhook-help" class="text-sm text-muted-foreground">
					{selectedFormat?.url_help}
				</p>
			</div>
			<Button
				type="submit"
				disabled={pending ||
					!format ||
					!eventKind ||
					!name.trim() ||
					!url.trim()}>{pending ? 'Working...' : 'Add webhook'}</Button
			>
		</form>
	</Panel>
	<Panel title="Your webhooks">
		{#if !webhooks.length}<p class="text-sm text-muted-foreground">
				You haven’t added any webhooks.
			</p>{/if}
		{#each webhooks as webhook (webhook.id)}
			<div
				class="flex flex-wrap items-center justify-between gap-3 border-b py-3 last:border-0"
			>
				<div>
					<p class="font-medium">{webhook.name}</p>
					<p class="text-sm text-muted-foreground">
						{data.options?.formats.find(
							(option) => option.value === webhook.format,
						)?.label ?? webhook.format} · {data.options?.events.find(
							(option) => option.value === webhook.event_kind,
						)?.label ?? webhook.event_kind} · {webhook.enabled
							? 'Active'
							: 'Paused'}
					</p>
				</div>
				<div class="flex gap-2">
					<Button
						variant="outline"
						disabled={pending}
						onclick={() => toggle(webhook)}
						>{webhook.enabled ? 'Pause' : 'Resume'}</Button
					>
					<Button
						variant="outline"
						disabled={pending}
						onclick={() => {
							target = webhook;
							confirmOpen = true;
						}}>Delete</Button
					>
				</div>
			</div>
		{/each}
		<p class="text-sm text-muted-foreground">
			Pausing stops new notifications and holds queued notifications until you
			resume. Deleting a webhook also removes its queued notifications.
		</p>
	</Panel>
</div>
{#if data.notifications}
	<div class="mt-6 max-w-2xl">
		<Panel title="Notification deliveries">
			{#if data.notifications.items.length === 0}
				<p class="text-sm text-muted-foreground">No notifications yet.</p>
			{:else}
				<div class="divide-y">
					{#each data.notifications.items as notification (notification.id)}
						<div class="space-y-1 py-3 first:pt-0 last:pb-0">
							<div class="flex flex-wrap justify-between gap-2">
								<p class="font-medium">
									{eventLabel(notification.event_kind)} to {notification.webhook_name}
								</p>
								<p class="text-sm">{statusLabel(notification.status)}</p>
							</div>
							<p class="text-sm text-muted-foreground">
								Created {dateTime(notification.created_at)} · {notification.attempt_count}
								{notification.attempt_count === 1 ? 'attempt' : 'attempts'}
								{#if notification.next_attempt_at}
									· Next try {dateTime(notification.next_attempt_at)}{/if}
								{#if notification.delivered_at}
									· Delivered {dateTime(notification.delivered_at)}{/if}
								{#if notification.failed_at}
									· Failed {dateTime(notification.failed_at)}{/if}
							</p>
							{#if notification.error_detail}
								<p class="text-sm text-destructive">
									{notification.error_detail}
								</p>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
			<PaginationControls
				pagination={data.notifications.pagination}
				label="notifications"
			/>
		</Panel>
	</div>
{/if}
<Dialog.Root bind:open={confirmOpen}>
	<Dialog.Content>
		<Dialog.Header
			><Dialog.Title>Delete webhook?</Dialog.Title><Dialog.Description
				>Delete “{target?.name}” and its queued notifications? You can add the
				destination again later.</Dialog.Description
			></Dialog.Header
		>
		{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
		<Dialog.Footer>
			<Button
				variant="outline"
				disabled={pending}
				onclick={() => {
					confirmOpen = false;
				}}>Cancel</Button
			>
			<Button variant="destructive" disabled={pending} onclick={remove}
				>Delete webhook</Button
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
