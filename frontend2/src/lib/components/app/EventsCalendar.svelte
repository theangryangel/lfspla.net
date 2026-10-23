<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Calendar,
		type EventInput,
		type EventSourceFuncInfo,
	} from 'fullcalendar';
	import dayGridPlugin from 'fullcalendar/daygrid';
	import listPlugin from 'fullcalendar/list';
	import classicTheme from 'fullcalendar/themes/classic';
	import enGbLocale from 'fullcalendar/locales/en-gb';
	import 'fullcalendar/skeleton.css';
	import 'fullcalendar/themes/classic/theme.css';
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { Button } from '#lib/components/ui/button/index.js';
	import { useApi } from '#lib/api.js';
	import { dateTime } from '#lib/format.js';
	import Empty from './Empty.svelte';
	import Choices from './Choices.svelte';

	let { leagueId, seasonId }: { leagueId?: number; seasonId?: number } =
		$props();
	const api = useApi();
	let element: HTMLDivElement;
	let calendar = $state<Calendar>();
	let title = $state('');
	let view = $state('dayGridWeek');
	const period = $derived(view.endsWith('Week') ? 'week' : 'month');
	const presentation = $derived(view.startsWith('list') ? 'list' : 'calendar');
	let loading = $state(true);
	let failed = $state(false);
	let count = $state(0);

	function changeView(
		nextPeriod: 'month' | 'week',
		nextPresentation: 'calendar' | 'list',
	) {
		const prefix = nextPresentation === 'calendar' ? 'dayGrid' : 'list';
		const suffix = nextPeriod === 'week' ? 'Week' : 'Month';
		calendar?.changeView(`${prefix}${suffix}`);
	}

	onMount(() => {
		let disposed = false;
		let request = 0;
		const instance = new Calendar(element, {
			plugins: [classicTheme, dayGridPlugin, listPlugin],
			locales: [enGbLocale],
			locale: 'en-gb',
			timeZone: 'local',
			initialView: window.matchMedia('(min-width: 768px)').matches
				? 'dayGridWeek'
				: 'listWeek',
			views: {
				dayGridWeek: {
					titleFormat: { day: 'numeric', month: 'short', year: 'numeric' },
				},
				listWeek: {
					titleFormat: { day: 'numeric', month: 'short', year: 'numeric' },
				},
			},
			headerToolbar: false,
			height: 'auto',
			borderless: true,
			viewClass: 'calendar-view',
			dayCellClass: 'calendar-day',
			dayHeaderClass: 'calendar-day-heading',
			dayHeaderInnerClass: 'calendar-day-heading-content',
			dayHeaderRowClass: 'calendar-weekdays',
			dayRowClass: 'calendar-week',
			listDayClass: 'calendar-list-day',
			listDayHeaderClass: 'calendar-list-heading',
			listDayHeaderInnerClass: 'calendar-list-heading-content',
			eventInnerClass: 'calendar-event-content',
			listItemEventBeforeClass: 'calendar-event-marker',
			eventClass: (info) =>
				info.event.extendedProps.lastInDay
					? 'calendar-event calendar-event-last'
					: 'calendar-event',
			firstDay: 1,
			fixedWeekCount: false,
			lazyFetching: false,
			editable: false,
			selectable: false,
			displayEventEnd: false,
			eventOrder: 'start,calendarOrder',
			eventTimeFormat: { hour: '2-digit', minute: '2-digit', hour12: false },
			listDayFormat: { day: 'numeric', month: 'short', year: 'numeric' },
			listDayAltFormat: false,
			// Empty and failed loads use our shared component below the calendar.
			noEventsClass: 'hidden',
			datesSet(info) {
				title = info.view.title;
				view = info.view.type;
			},
			async events(info: EventSourceFuncInfo): Promise<EventInput[]> {
				const current = ++request;
				loading = true;
				failed = false;
				try {
					const filter = {
						starts_from: info.start.toISOString(),
						starts_before: info.end.toISOString(),
						league_id: leagueId,
						season_id: seasonId,
						per_page: 100,
					};
					const first = await api.events.listEvents({ ...filter, page: 1 });
					const items = [...first.items];
					for (let page = 2; page <= first.pagination.total_pages; page += 1) {
						if (disposed || current !== request) return [];
						const response = await api.events.listEvents({ ...filter, page });
						items.push(...response.items);
					}
					if (!disposed && current === request) count = items.length;
					const lastByDay = new Map<string, number>();
					for (const event of items) {
						lastByDay.set(new Date(event.starts_at).toDateString(), event.id);
					}
					return items.map((event, index) => ({
						id: String(event.id),
						title: [event.league.name, event.season?.name, event.title]
							.filter(Boolean)
							.join(' / '),
						start: event.starts_at,
						allDay: false,
						url: event.details_url ?? undefined,
						extendedProps: {
							calendarOrder: index,
							lastInDay:
								lastByDay.get(new Date(event.starts_at).toDateString()) ===
								event.id,
							imageUrl: event.league.image?.url,
							leagueName: event.league.name,
							eventName: [event.season?.name, event.title]
								.filter(Boolean)
								.join(' / '),
						},
					}));
				} catch {
					if (!disposed && current === request) {
						failed = true;
						count = 0;
					}
					return [];
				} finally {
					if (!disposed && current === request) loading = false;
				}
			},
			eventContent(info) {
				// Imported content is always text; never pass upstream HTML to FullCalendar.
				const isList = info.view.type.startsWith('list');
				const content = document.createElement('div');
				content.className = isList
					? 'flex w-full min-w-0 items-center gap-4'
					: 'flex w-full min-w-0 items-start gap-2';
				let logo: HTMLImageElement | undefined;
				if (info.event.extendedProps.imageUrl) {
					const image = document.createElement('img');
					image.src = info.event.extendedProps.imageUrl;
					image.alt = '';
					if (isList) {
						image.width = 64;
						image.height = 64;
					}
					image.loading = 'lazy';
					image.decoding = 'async';
					image.className = isList
						? 'size-16 shrink-0 object-contain'
						: 'my-2 aspect-video w-full object-contain';
					image.onerror = () => image.remove();
					logo = image;
					if (isList) content.append(image);
				}
				const text = document.createElement('div');
				text.className = 'min-w-0 flex-1 whitespace-normal wrap-anywhere';
				if (info.timeText) {
					const time = document.createElement('time');
					time.className = 'block text-muted-foreground tabular-nums';
					if (info.event.start) time.dateTime = info.event.start.toISOString();
					time.textContent = info.timeText;
					text.append(time);
				}
				if (!isList && logo) text.append(logo);
				const name = document.createElement('span');
				name.className = 'block font-medium';
				name.textContent = isList
					? info.event.title
					: info.event.extendedProps.leagueName;
				text.append(name);
				if (!isList) {
					const detail = document.createElement('span');
					detail.className = 'block text-muted-foreground';
					detail.textContent = info.event.extendedProps.eventName;
					text.append(detail);
				}
				content.append(text);
				return { domNodes: [content] };
			},
			eventDidMount(info) {
				info.el.title = info.event.start
					? `${info.event.title}\n${dateTime(info.event.start.toISOString())}`
					: info.event.title;
				const links =
					info.el instanceof HTMLAnchorElement
						? [info.el]
						: info.el.querySelectorAll<HTMLAnchorElement>('a[href]');
				for (const link of links) {
					link.target = '_blank';
					link.rel = 'noopener noreferrer';
				}
			},
		});
		calendar = instance;
		instance.render();
		return () => {
			disposed = true;
			instance.destroy();
		};
	});
</script>

<section
	class="event-calendar racing-data min-w-0 space-y-4 text-sm"
	aria-label="Event calendar"
	aria-busy={loading}
>
	<div class="flex flex-wrap items-center justify-between gap-3">
		<div class="flex items-center gap-0.5">
			<Button
				variant="ghost"
				class="pl-1.5!"
				aria-label={`Previous ${period}`}
				onclick={() => calendar?.prev()}
			>
				<ChevronLeft data-icon="inline-start" aria-hidden="true" />
				<span class="hidden sm:block">Previous</span>
			</Button>
			<Button variant="ghost" onclick={() => calendar?.today()}>Today</Button>
			<Button
				variant="ghost"
				class="pr-1.5!"
				aria-label={`Next ${period}`}
				onclick={() => calendar?.next()}
			>
				<span class="hidden sm:block">Next</span>
				<ChevronRight data-icon="inline-end" aria-hidden="true" />
			</Button>
		</div>
		<h2 class="racing-section-title" aria-live="polite">{title}</h2>
		<div class="flex flex-wrap items-center gap-3">
			<Choices
				label="Date range"
				value={period === 'week' ? 'Week' : 'Month'}
				options={['Month', 'Week']}
				onValueChange={(value) =>
					changeView(value === 'Week' ? 'week' : 'month', presentation)}
			/>
			<Choices
				label="Calendar presentation"
				value={presentation === 'list' ? 'List' : 'Calendar'}
				options={['Calendar', 'List']}
				onValueChange={(value) =>
					changeView(period, value === 'List' ? 'list' : 'calendar')}
			/>
		</div>
	</div>
	{#if loading}<p role="status" class="text-muted-foreground">
			Loading events...
		</p>{/if}
	<div class="calendar-surface" bind:this={element}></div>
	{#if !loading && failed}
		<Empty title="Events couldn’t be loaded">
			<p>Please try again.</p>
			<Button variant="outline" onclick={() => calendar?.refetchEvents()}
				>Retry</Button
			>
		</Empty>
	{:else if !loading && count === 0}
		<Empty title={`No events this ${period}`}
			><p>Try another {period} to find your next race.</p></Empty
		>
	{/if}
</section>

<style>
	.event-calendar {
		--fc-classic-background: var(--card);
		--fc-classic-foreground: var(--foreground);
		--fc-classic-border: var(--border);
		--fc-classic-strong-border: var(--ring);
		--fc-classic-faint: color-mix(in oklch, var(--muted) 50%, transparent);
		--fc-classic-muted: var(--muted);
		--fc-classic-strong: var(--accent);
		--fc-classic-faint-foreground: var(--muted-foreground);
		--fc-classic-muted-foreground: var(--muted-foreground);
		--fc-classic-primary: var(--primary);
		--fc-classic-primary-foreground: var(--primary-foreground);
		--fc-classic-event: var(--primary);
		--fc-classic-event-contrast: var(--primary-foreground);
		--fc-classic-today: color-mix(in oklch, var(--primary) 10%, transparent);
		--fc-classic-highlight: var(--accent);
		--fc-classic-now: var(--primary);
		--fc-classic-button: var(--background);
		--fc-classic-button-border: var(--border);
		--fc-classic-button-strong: var(--muted);
		--fc-classic-button-strong-border: var(--border);
		--fc-classic-button-outline: var(--ring);
		--fc-classic-button-foreground: var(--foreground);
	}
	.event-calendar :global(.calendar-surface) {
		font-family: inherit;
		font-size: inherit;
		line-height: inherit;
	}
	.event-calendar :global(.calendar-event-content),
	.event-calendar :global(.calendar-day-heading-content),
	.event-calendar :global(.calendar-list-heading-content) {
		font-size: inherit;
		line-height: inherit;
	}
	.event-calendar :global(.calendar-view),
	.event-calendar :global(.calendar-list-day) {
		border: 0;
	}
	.event-calendar :global(.calendar-day),
	.event-calendar :global(.calendar-day-heading) {
		border-inline-width: 0;
	}
	.event-calendar :global(.calendar-day + .calendar-day),
	.event-calendar :global(.calendar-day-heading + .calendar-day-heading) {
		border-inline-start: 1px solid var(--border);
	}
	.event-calendar :global(.calendar-week),
	.event-calendar :global(.calendar-weekdays) {
		border-inline-width: 0;
		border-bottom: 1px solid var(--border);
	}
	.event-calendar :global(.calendar-day-heading) {
		color: var(--muted-foreground);
		font-weight: 500;
	}
	.event-calendar :global(.calendar-list-heading) {
		background: color-mix(in oklch, var(--muted) 50%, transparent);
		border: 0;
		border-bottom: 1px solid var(--border);
	}
	.event-calendar :global(.calendar-list-heading[aria-current='date']) {
		background: var(--fc-classic-today);
		color: var(--primary);
	}
	.event-calendar :global(.calendar-event-marker) {
		display: none;
	}
	.event-calendar :global(.calendar-event) {
		border: 0;
		border-bottom: 1px solid var(--border);
		border-radius: 0;
		background: transparent;
		color: var(--foreground);
		padding: 0.5rem;
		transition: background-color 150ms;
	}
	.event-calendar :global(.calendar-event:hover) {
		background: color-mix(in oklch, var(--muted) 50%, transparent);
	}
	.event-calendar :global(.calendar-event-last) {
		border-bottom: 0;
	}
	.event-calendar :global(a:focus-visible) {
		outline: 2px solid var(--ring);
		outline-offset: -2px;
	}
</style>
