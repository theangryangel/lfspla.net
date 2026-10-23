<script lang="ts">
	import type { HTMLTimeAttributes } from 'svelte/elements';
	import { date, dateTime, time } from '#lib/format.js';

	let {
		value,
		format = 'datetime',
		title,
		...attributes
	}: Omit<HTMLTimeAttributes, 'datetime' | 'children'> & {
		value: string;
		format?: 'date' | 'datetime' | 'time';
	} = $props();
</script>

<time
	{...attributes}
	datetime={value}
	title={title ?? (format !== 'datetime' ? dateTime(value) : undefined)}
	>{format === 'date'
		? date(value)
		: format === 'time'
			? time(value)
			: dateTime(value)}</time
>
