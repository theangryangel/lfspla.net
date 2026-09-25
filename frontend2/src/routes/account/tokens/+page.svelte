<script lang="ts">
	import {
		send,
		type CreatedPersonalAccessTokenResponse,
		type PersonalAccessTokenResponse,
	} from '$lib/api.js';
	import { dateTime } from '$lib/format.js';
	import { useSession } from '$lib/session.svelte.js';
	import Panel from '$lib/components/app/Panel.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const session = useSession();
	const endpoint = '/api/v1/me/personal-access-tokens';
	let tokens = $derived(data.tokens);
	let name = $state('');
	let days = $state<number | undefined>(90);
	let pending = $state(false);
	let error = $state('');
	let created = $state<CreatedPersonalAccessTokenResponse | null>(null);
	let copyMessage = $state('');
	let notice = $state('');
	let target = $state<PersonalAccessTokenResponse | null>(null);
	let confirmOpen = $state(false);
	let revokeError = $state('');

	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (pending || created) return;
		pending = true;
		error = '';
		notice = '';
		copyMessage = '';
		try {
			const result = await send<CreatedPersonalAccessTokenResponse>(endpoint, {
				method: 'POST',
				csrf: session.me.csrf_token,
				json: { name: name.trim(), expires_in_days: days },
			});
			if (!result)
				throw new Error(
					'The server did not return the new token. Reload the list before trying again.',
				);
			created = result;
			tokens = [result.credential, ...tokens];
			name = '';
		} catch (cause) {
			error =
				cause instanceof Error
					? cause.message
					: 'The token could not be created.';
		} finally {
			pending = false;
		}
	}
	async function copy() {
		if (!created) return;
		try {
			await navigator.clipboard.writeText(created.token);
			copyMessage = 'Token copied.';
		} catch {
			copyMessage = 'Copying failed. Select the token and copy it manually.';
		}
	}
	async function revoke() {
		if (!target || pending) return;
		pending = true;
		revokeError = '';
		const token = target;
		try {
			await send(`${endpoint}/${token.id}`, {
				method: 'DELETE',
				csrf: session.me.csrf_token,
			});
			tokens = tokens.map((item) =>
				item.id === token.id
					? { ...item, revoked_at: new Date().toISOString() }
					: item,
			);
			if (created?.credential.id === token.id) created = null;
			notice = `“${token.name}” was revoked.`;
			confirmOpen = false;
		} catch (cause) {
			revokeError =
				cause instanceof Error
					? cause.message
					: 'The token could not be revoked.';
		} finally {
			pending = false;
		}
	}
</script>

<svelte:head><title>Personal access tokens · lfspla.net</title></svelte:head>
<p class="text-sm text-muted-foreground">
	Create credentials for tools that use the API on your behalf. Keep them
	private.
</p>
<div class="max-w-2xl">
	<Panel title="Create a token">
		<form onsubmit={create} class="space-y-4">
			<div class="grid gap-4 sm:grid-cols-[1fr_10rem]">
				<div class="space-y-2">
					<label for="token-name" class="text-sm font-medium">Name</label>
					<Input
						id="token-name"
						bind:value={name}
						required
						maxlength={100}
						placeholder="e.g. Replay uploader"
						disabled={pending || !!created}
					/>
				</div>
				<div class="space-y-2">
					<label for="token-days" class="text-sm font-medium"
						>Expires in (days)</label
					>
					<Input
						id="token-days"
						type="number"
						bind:value={days}
						required
						min={1}
						max={365}
						step={1}
						disabled={pending || !!created}
					/>
				</div>
			</div>
			{#if error}<p role="alert" class="text-sm text-destructive">
					{error}
				</p>{/if}
			<Button type="submit" disabled={pending || !!created || !name.trim()}
				>{pending ? 'Working...' : 'Create token'}</Button
			>
		</form>
		{#if created}
			<div class="space-y-3 rounded-lg border border-primary p-4">
				<p role="status" class="font-medium">Your token is ready</p>
				<p class="text-sm text-muted-foreground">
					Copy it now. You won’t be able to view it again after leaving this
					page or dismissing it.
				</p>
				<label for="new-token" class="sr-only">New personal access token</label>
				<Input
					id="new-token"
					readonly
					value={created.token}
					class="font-mono"
					onclick={(event) => event.currentTarget.select()}
				/>
				<div class="flex flex-wrap gap-2">
					<Button variant="outline" onclick={copy}>Copy token</Button>
					<Button
						variant="ghost"
						onclick={() => {
							created = null;
							copyMessage = '';
						}}>I’ve saved it</Button
					>
				</div>
				{#if copyMessage}<p role="status" class="text-sm">{copyMessage}</p>{/if}
			</div>
		{/if}
	</Panel>
</div>
{#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
<Panel title="Your tokens" flush={tokens.length > 0}>
	{#if tokens.length === 0}
		<p class="text-sm text-muted-foreground">
			You haven’t created any personal access tokens yet.
		</p>
	{:else}
		<Table.Root>
			<Table.Header
				><Table.Row>
					<Table.Head>Name</Table.Head><Table.Head>Status</Table.Head
					><Table.Head>Created</Table.Head><Table.Head>Expires</Table.Head
					><Table.Head>Last used</Table.Head><Table.Head
						><span class="sr-only">Actions</span></Table.Head
					>
				</Table.Row></Table.Header
			>
			<Table.Body>
				{#each tokens as token (token.id)}
					{@const expired = new Date(token.expires_at).getTime() <= Date.now()}
					<Table.Row>
						<Table.Cell
							><div class="font-medium">{token.name}</div>
							<code class="text-xs text-muted-foreground"
								>{token.token_hint}</code
							></Table.Cell
						>
						<Table.Cell
							>{token.revoked_at
								? 'Revoked'
								: expired
									? 'Expired'
									: 'Active'}</Table.Cell
						>
						<Table.Cell>{dateTime(token.created_at)}</Table.Cell>
						<Table.Cell>{dateTime(token.expires_at)}</Table.Cell>
						<Table.Cell
							>{token.last_used_at
								? dateTime(token.last_used_at)
								: 'Never'}</Table.Cell
						>
						<Table.Cell>
							{#if !token.revoked_at && !expired}
								<Button
									variant="outline"
									size="sm"
									disabled={pending}
									aria-label={`Revoke ${token.name}`}
									onclick={() => {
										target = token;
										revokeError = '';
										confirmOpen = true;
									}}>Revoke</Button
								>
							{/if}
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	{/if}
</Panel>
<Dialog.Root bind:open={confirmOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Revoke token?</Dialog.Title>
			<Dialog.Description
				>Tools using “{target?.name}” will lose access immediately. This cannot
				be undone.</Dialog.Description
			>
		</Dialog.Header>
		{#if revokeError}<p role="alert" class="text-sm text-destructive">
				{revokeError}
			</p>{/if}
		<Dialog.Footer>
			<Button
				variant="outline"
				disabled={pending}
				onclick={() => {
					confirmOpen = false;
				}}>Cancel</Button
			>
			<Button variant="destructive" disabled={pending} onclick={revoke}
				>{pending ? 'Revoking...' : 'Revoke token'}</Button
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
