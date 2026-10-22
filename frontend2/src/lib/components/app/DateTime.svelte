<script lang="ts">
	import type { HTMLTimeAttributes } from 'svelte/elements';
	import { date, dateTime } from '#lib/format.js';

	let {
		value,
		format = 'datetime',
		title,
		...attributes
	}: Omit<HTMLTimeAttributes, 'datetime' | 'children'> & {
		value: string;
		format?: 'date' | 'datetime';
	} = $props();
</script>

<time
	{...attributes}
	datetime={value}
	title={title ?? (format === 'date' ? dateTime(value) : undefined)}
	>{format === 'date' ? date(value) : dateTime(value)}</time
>
