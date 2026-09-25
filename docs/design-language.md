# Design language

Keep the interface calm, compact, and useful. Show racing data first. Use colour
for actions, status, and identity. These rules apply to `frontend2`.

## Principles

- Show values, tables, rankings, and useful metadata before decoration.
- Keep the home page brief. Show activity and totals straight away.
- Use spacing and type before colour.
- Keep navigation quiet and clear.
- Do not use colour as the only signal.
- Keep shadcn-svelte's rounded controls, neutral surfaces, borders, and focus states.

## Colours

### Primary: cyan

`primary` is for important actions and information.

- Primary form actions, link buttons, selected controls, notices, and progress.
- Do not use it for navigation hover or active states; use `accent`.
- Focus uses `ring`, not `primary`.

```css
--primary: oklch(0.779 0.136 233);
```

### Accent: neutral

`accent` marks active navigation, menu items, selected rows, and quiet
selections. Use `accent-foreground` for active navigation text.

Ghost buttons use `muted` on hover. Table hover uses `muted/50`.

```css
/* Light */
--accent: oklch(0.97 0 0);
--accent-foreground: oklch(0.205 0 0);

/* Dark */
--accent: oklch(0.269 0 0);
--accent-foreground: oklch(0.985 0 0);
```

### Brand yellow

Yellow is for the `.net` wordmark and the Upload hotlap trigger. Do not use it
for links, navigation, focus, or normal form submission. The upload dialog's
submit button is cyan.

### Status colours

- `time-best` (purple): records, first place, and the faster comparison result.
- `time-pb` (green): open-era labels and accepted-upload counts.
- `destructive` (red): rejected, failed, delete, and revoke actions.

Status also needs text, labels, icons, or layout changes.

Player badges have their own colours. Top three standings use podium colours;
the rest use slate.

## Actions

Use the least prominent control that makes the action clear:

1. **Primary**: main action; cyan by default.
2. **Outline**: important secondary action.
3. **Secondary**: visible support action.
4. **Ghost**: utility action, dismissal, or toolbar control.
5. **Link**: cyan text with an underline on hover.

Use **Destructive** for delete and revoke confirmations. One compact toolbar
should usually have one strongest action.

## Navigation

- Use `text-sm font-medium`, rounded corners, and compact padding.
- Use `accent` and `accent-foreground` for active items.
- Keep the `ring` focus state.
- Keep breadcrumbs quieter than page actions.
- The wordmark links home. Do not add a separate Home item.

`PageNavigation` also has a `prominent` variant with a cyan underline. Current
routes use the neutral default.

## Layout

- Use Geist for text, `tabular-nums` for aligned numbers, and `font-mono` for times.
- Use `background` for pages, `card` for contained content, and `muted` for quiet surfaces.
- Use rows, dividers, and tables for compact data.
- Use borders for structure.
- Align toolbars to `h-8` beside standard buttons.
- Data pages can use full width; keep the site shell readable.

The main shell uses `max-w-screen-2xl` with `p-4` / `md:p-6`.

## Accessibility

- Check contrast after changing a token or custom colour.
- Keep visible keyboard focus.
- Do not rely on colour alone.
- Give icon-only controls useful labels.

Player badges need a separate accessibility review before claiming every status
has an accessible label.
