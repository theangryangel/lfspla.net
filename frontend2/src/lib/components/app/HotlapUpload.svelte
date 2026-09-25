<script lang="ts">
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import CircleCheckBigIcon from '@lucide/svelte/icons/circle-check-big';
	import FileUpIcon from '@lucide/svelte/icons/file-up';
	import FilesIcon from '@lucide/svelte/icons/files';
	import LoaderCircleIcon from '@lucide/svelte/icons/loader-circle';
	import RotateCcwIcon from '@lucide/svelte/icons/rotate-ccw';
	import UploadIcon from '@lucide/svelte/icons/upload';
	import XIcon from '@lucide/svelte/icons/x';
	import { invalidate } from '$app/navigation';
	import { onMount } from 'svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import ChoiceSelect from '$lib/components/app/ChoiceSelect.svelte';
	import { fileSize, hotlapStates, lapTime } from '$lib/format.js';
	import { readSprHeader, type SprSummary } from '$lib/spr.js';
	import {
		RequestFailed,
		send,
		type EraSummary,
		type ManagedHotlapResponse,
		type HotlapState,
	} from '$lib/api.js';
	import { useSession } from '$lib/session.svelte.js';

	let {
		era,
		eras,
	}: {
		era: EraSummary;
		eras?: EraSummary[];
	} = $props();

	const CONCURRENCY = 2;
	const REFRESH_SECONDS = 10;

	type Status = 'queued' | 'uploading' | 'accepted' | 'failed';

	interface Item {
		id: number;
		file: File;
		status: Status;
		header: SprSummary | null;
		error: string | null;
		retryable: boolean;
		elsewhere: { id: string; title: string } | null;
		eraId: string | null;
		hotlap: ManagedHotlapResponse | null;
	}

	const session = useSession();
	const maxBytes = $derived(session.me.max_spr_upload_bytes);

	let open = $state(false);
	let items = $state<Item[]>([]);
	let running = $state(false);
	let refreshError = $state('');
	let dragging = $state(false);
	let duplicates = $state(0);
	let refreshCountdown = $state(REFRESH_SECONDS);
	let chosen = $state<string | null>(null);
	let nextId = 1;
	// Child dragleave events also reach the dropzone.
	let dragDepth = 0;
	const pendingUploads = () =>
		items.filter(
			(item) => item.status === 'accepted' && item.hotlap?.state === 'pending',
		);

	async function refreshPending() {
		const pending = pendingUploads();
		const results = await Promise.all(
		pending.map(async (item) => {
			try {
				const detail = await send<{ state: HotlapState }>(
					`/api/v1/hotlaps/${item.hotlap!.id}`,
					{ method: 'GET', csrf: session.me.csrf_token },
				);
				if (!detail) return false;
				if (item.hotlap) item.hotlap.state = detail.state;
				return detail.state !== 'pending';
				} catch {
					// Keep the current state and try again on the next refresh.
					return false;
				}
			}),
		);
		if (results.some(Boolean)) {
			try {
				await invalidate('app:hotlaps');
			} catch {
				refreshError = 'The page could not refresh. Please reload.';
			}
		}
	}

	onMount(() => {
		const timer = setInterval(() => {
			if (!pendingUploads().length) {
				refreshCountdown = REFRESH_SECONDS;
				return;
			}
			if (refreshCountdown > 1) {
				refreshCountdown -= 1;
				return;
			}
			refreshCountdown = REFRESH_SECONDS;
			void refreshPending();
		}, 1000);
		return () => clearInterval(timer);
	});

	// Closed eras cannot accept uploads.
	const targets = $derived(
		(eras ?? [era]).filter((candidate) => candidate.open),
	);
	const target = $derived(
		targets.find((candidate) => candidate.id === (chosen ?? era.id)) ??
			targets[0],
	);
	const targetOptions = $derived(
		targets.map((e) => ({ value: e.id, label: e.title })),
	);

	const counts = $derived({
		queued: items.filter((item) => item.status === 'queued').length,
		accepted: items.filter((item) => item.status === 'accepted').length,
		uploading: items.filter((item) => item.status === 'uploading').length,
		failed: items.filter((item) => item.status === 'failed').length,
	});
	const retryable = $derived(
		items.filter((item) => item.status === 'failed' && item.retryable).length,
	);
	const eraTitle = (id: string) =>
		(eras ?? [era]).find((candidate) => candidate.id === id)?.title ?? id;
	const replays = (count: number) =>
		`${count} ${count === 1 ? 'replay' : 'replays'}`;

	const key = (file: File) =>
		`${file.name}\0${file.size}\0${file.lastModified}`;

	function reject(file: File): string | null {
		if (!file.name.toLowerCase().endsWith('.spr'))
			return 'Only .spr replay files are accepted.';
		if (file.size === 0) return 'This file is empty.';
		if (file.size > maxBytes)
			return `This file is over the ${fileSize(maxBytes)} upload limit.`;
		return null;
	}

	async function add(files: FileList | null | undefined) {
		const selected = Array.from(files ?? []);
		if (!selected.length) return;
		const known = new Set(items.map((item) => key(item.file)));
		let repeated = 0;

		for (const file of selected) {
			if (known.has(key(file))) {
				repeated += 1;
				continue;
			}
			known.add(key(file));
			let error = reject(file);
			let header: SprSummary | null = null;
			if (!error) {
				try {
					header = await readSprHeader(file);
				} catch {
					error = 'This file could not be read. Please select it again.';
				}
			}
			items.push({
				id: nextId++,
				file,
				status: error || !header ? 'failed' : 'queued',
				header,
				error: error ?? (header ? null : 'This file is not an LFS replay.'),
				retryable: false,
				elsewhere: null,
				eraId: null,
				hotlap: null,
			});
		}
		duplicates = repeated;
	}

	function elsewhere(
		failure: RequestFailed,
	): { id: string; title: string } | null {
		if (failure.code !== 'wrong_era') return null;
		const details = failure.details as
			{ era_id?: string; era_title?: string } | undefined;
		if (!details?.era_id) return null;
		return { id: details.era_id, title: details.era_title ?? details.era_id };
	}

	async function uploadOne(id: number) {
		const item = items.find((candidate) => candidate.id === id);
		if (!item || item.status !== 'queued') return;
		item.status = 'uploading';
		item.error = null;
		item.retryable = false;
		item.elsewhere = null;

		const body = new FormData();
		body.append('spr', item.file, item.file.name);
		try {
			item.hotlap = await send<ManagedHotlapResponse>(
				`/api/v1/eras/${encodeURIComponent(item.eraId ?? target.id)}/hotlaps`,
				{ method: 'POST', csrf: session.me.csrf_token, body },
			);
			item.status = 'accepted';
		} catch (failure) {
			item.status = 'failed';
			item.error =
				failure instanceof RequestFailed
					? failure.message
					: 'The replay could not be uploaded.';
			item.retryable = failure instanceof RequestFailed && failure.retryable;
			item.elsewhere =
				failure instanceof RequestFailed ? elsewhere(failure) : null;
		}
	}

	// Upload independently; refresh once.
	async function upload(ids?: number[]) {
		if (running) return;
		const queue =
			ids ??
			items.filter((item) => item.status === 'queued').map((item) => item.id);
		if (!queue.length) return;

		running = true;
		refreshError = '';
		let cursor = 0;
		const worker = async () => {
			while (cursor < queue.length) await uploadOne(queue[cursor++]);
		};
		try {
			await Promise.all(
				Array.from({ length: Math.min(CONCURRENCY, queue.length) }, worker),
			);
		} finally {
			running = false;
		}
		if (
			items.some(
				(item) => queue.includes(item.id) && item.status === 'accepted',
			)
		) {
			refreshCountdown = REFRESH_SECONDS;
			try {
				await invalidate('app:hotlaps');
			} catch {
				refreshError =
					'Your replays were uploaded, but the page could not refresh. Please reload.';
			}
		}
	}

	function retry(ids: number[], eraId: string | null = null) {
		for (const id of ids) {
			const item = items.find((candidate) => candidate.id === id);
			if (!item) continue;
			item.status = 'queued';
			item.error = null;
			item.retryable = false;
			if (eraId) item.eraId = eraId;
		}
		void upload(ids);
	}

	function drop(event: DragEvent) {
		event.preventDefault();
		dragDepth = 0;
		dragging = false;
		void add(event.dataTransfer?.files);
	}
</script>

{#if targets.length}
	<Dialog.Root bind:open>
		<Dialog.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					class="!cursor-pointer bg-[oklch(0.852_0.199_91.936)] text-zinc-950 hover:bg-[oklch(0.795_0.184_86.047)]"
				>
					<UploadIcon />
					Upload hotlap
				</Button>
			{/snippet}
		</Dialog.Trigger>
		<Dialog.Content class="sm:max-w-2xl">
			<Dialog.Header>
				<Dialog.Title>Upload replays to {target.title}</Dialog.Title>
				<Dialog.Description>
					The era accepts replays recorded with LFS {target.version_requirement}.
					Each one is published once its vehicle is resolved and HLVC has
					validated it.
				</Dialog.Description>
			</Dialog.Header>
			{#if refreshError}<p role="alert" class="text-destructive">
					{refreshError}
				</p>{/if}

			{#if !session.signedIn}
				<p class="text-sm text-muted-foreground">
					Hotlaps are attached to your LFS account, so uploading needs you
					signed in first.
				</p>
				<Dialog.Footer>
					<Button onclick={session.signIn}>Sign in with LFS</Button>
				</Dialog.Footer>
			{:else if !session.me.allow_uploads}
				<p class="text-sm text-muted-foreground">
					Your account is not currently allowed to upload hotlaps.
				</p>
			{:else}
				{#if targets.length > 1}
					<ChoiceSelect
						label="Era"
						value={target.id}
						options={targetOptions}
						onValueChange={(value) => (chosen = value)}
					/>
				{/if}

				<label
					for="spr-files"
					class="flex cursor-pointer flex-col items-center gap-2 rounded-lg border border-dashed px-5 py-6 text-center transition-colors {dragging
						? 'border-primary bg-primary/5'
						: 'hover:bg-muted/50'}"
					ondragenter={(e) => {
						if (!e.dataTransfer?.types.includes('Files')) return;
						dragDepth += 1;
						dragging = true;
					}}
					ondragover={(e) => e.preventDefault()}
					ondragleave={() => {
						dragDepth = Math.max(0, dragDepth - 1);
						dragging = dragDepth > 0;
					}}
					ondrop={drop}
				>
					<FilesIcon class="size-5 text-muted-foreground" />
					<span class="text-sm font-medium">
						{dragging
							? 'Drop your replays here'
							: 'Drop SPR files here or choose files'}
					</span>
					<span class="text-xs text-muted-foreground">
						One or more single-player replays · {fileSize(maxBytes)} each at most
					</span>
					<input
						id="spr-files"
						type="file"
						accept=".spr,application/octet-stream"
						multiple
						class="sr-only"
						onchange={(e) => {
							void add(e.currentTarget.files);
							e.currentTarget.value = '';
						}}
					/>
				</label>

				{#if duplicates}
					<p class="text-sm text-muted-foreground" role="status">
						{duplicates}
						{duplicates === 1 ? 'file was' : 'files were'} already in the queue.
					</p>
				{/if}

				{#if items.length}
					<div class="rounded-lg border">
						<div
							class="flex flex-wrap items-center justify-between gap-2 border-b px-4 py-2"
						>
							<p class="text-sm font-medium">
								{replays(items.length)} selected
							</p>
							{#if pendingUploads().length}
								<p class="text-xs text-muted-foreground" aria-live="polite">
									Refreshing pending uploads in {refreshCountdown}s
								</p>
							{/if}
							<div class="flex flex-wrap gap-1" aria-live="polite">
								{#if counts.queued}<Badge variant="outline"
										>{counts.queued} queued</Badge
									>{/if}
								{#if counts.uploading}
									<Badge variant="secondary">{counts.uploading} uploading</Badge
									>
								{/if}
								{#if counts.accepted}
									<Badge
										variant="outline"
										class="border-time-pb/40 text-time-pb"
										>{counts.accepted} accepted</Badge
									>
								{/if}
								{#if counts.failed}
									<Badge variant="destructive">{counts.failed} failed</Badge>
								{/if}
							</div>
						</div>
						<ul
							class="max-h-64 divide-y overflow-y-auto"
							aria-label="Replay upload queue"
						>
							{#each items as item (item.id)}
								<li class="flex items-start gap-3 px-4 py-2.5">
									<span class="mt-0.5 shrink-0 text-muted-foreground">
										{#if item.status === 'uploading'}
											<LoaderCircleIcon class="size-4 animate-spin" />
										{:else if item.status === 'accepted'}
											<CircleCheckBigIcon class="size-4 text-primary" />
										{:else if item.status === 'failed'}
											<CircleAlertIcon class="size-4 text-destructive" />
										{:else}
											<FileUpIcon class="size-4" />
										{/if}
									</span>
									<div class="min-w-0 flex-1">
										<p
											class="truncate text-sm font-medium"
											title={item.file.name}
										>
											{item.file.name}
										</p>
										<p class="mt-0.5 text-xs text-muted-foreground">
											{#if item.status === 'accepted' && item.hotlap}
												{eraTitle(item.hotlap.era_id)} · {item.hotlap.track} · {item
													.hotlap.vehicle ?? item.hotlap.raw_vehicle_name} · {lapTime(
													item.hotlap.lap_time_ms,
												)} ·
												{hotlapStates[item.hotlap.state].label}
											{:else if item.status === 'failed'}
												<span class="font-medium text-destructive"
													>{item.error}</span
												>
											{:else if item.status === 'uploading'}
												Uploading {fileSize(item.file.size)}...
											{:else if item.header}
												LFS {item.header.version} · {item.header.track} · {fileSize(
													item.file.size,
												)}
											{/if}
										</p>
									</div>
									<div class="flex shrink-0 items-center gap-1">
										{#if item.status === 'failed' && item.elsewhere}
											<Button
												variant="outline"
												size="sm"
												disabled={running}
												onclick={() => retry([item.id], item.elsewhere?.id)}
											>
												Upload to {item.elsewhere.title}
											</Button>
										{:else if item.status === 'failed' && item.retryable}
											<Button
												variant="outline"
												size="sm"
												disabled={running}
												onclick={() => retry([item.id])}
											>
												<RotateCcwIcon />
												Retry
											</Button>
										{/if}
										<Button
											variant="ghost"
											size="icon-sm"
											disabled={item.status === 'uploading'}
											aria-label="Remove {item.file.name} from the queue"
											onclick={() =>
												(items = items.filter((other) => other.id !== item.id))}
										>
											<XIcon />
										</Button>
									</div>
								</li>
							{/each}
						</ul>
					</div>
				{/if}

				<Dialog.Footer class="sm:justify-between">
					<div class="flex flex-wrap gap-2">
						{#if retryable > 1}
							<Button
								variant="secondary"
								disabled={running}
								onclick={() =>
									retry(
										items
											.filter(
												(item) => item.status === 'failed' && item.retryable,
											)
											.map((item) => item.id),
									)}
							>
								<RotateCcwIcon />
								Retry failed
							</Button>
						{/if}
						{#if counts.accepted}
							<Button
								variant="ghost"
								disabled={running}
								onclick={() =>
									(items = items.filter((item) => item.status !== 'accepted'))}
							>
								Clear accepted
							</Button>
						{/if}
					</div>
					<Button disabled={!counts.queued || running} onclick={() => upload()}>
						{#if running}
							<LoaderCircleIcon class="animate-spin" />
							Uploading...
						{:else}
							<FileUpIcon />
							{counts.queued
								? `Upload ${replays(counts.queued)}`
								: 'Upload replays'}
						{/if}
					</Button>
				</Dialog.Footer>
			{/if}
		</Dialog.Content>
	</Dialog.Root>
{/if}
