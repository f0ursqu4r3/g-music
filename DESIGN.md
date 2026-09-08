# G Music Design Guide

## Source

The Library window is the reference surface. It balances a dense collection view
with a calm playback environment. Other windows use the same tokens and shared
window utilities. They can change layout when their task needs it.

## Scene and register

G Music is a product interface for someone listening while they work, often in a
low-light desktop environment. The interface uses dark, tinted surfaces so music
controls remain visible without pulling focus from the primary task.

Use the product register. Favor familiar controls, compact spacing, and clear
active states. Do not add marketing copy, decorative cards, or unrelated visual
effects.

## Color

Use the semantic variables in `src/styles.css`.

| Variables                     | Purpose                          |
| ----------------------------- | -------------------------------- |
| `--canvas`                    | Immersive artwork canvas.        |
| `--glass-window`              | Standard window surface.         |
| `--surface`                   | Grouped panels.                  |
| `--glass-control`             | Inputs and secondary controls.   |
| `--line`                      | Standard dividers.               |
| `--line-strong`               | Emphasized boundaries.           |
| `--text`                      | Primary text.                    |
| `--muted-text`                | Supporting text.                 |
| `--subtle-text`               | Quiet labels and icons.          |
| `--accent`                    | Active state and primary action. |
| `--accent-soft`               | Selection fill.                  |
| `--danger`, `--danger-soft`   | Errors only.                     |
| `--warning`, `--warning-soft` | Warnings only.                   |

Do not introduce raw color values in window components when a semantic variable
fits. Themes redefine the color primitives and keep their semantic roles stable.

Popover and context-menu surfaces derive an opaque, raised tint from the active
window's `--glass-window` color. Keep `--menu-surface` linked to `--popover` so
portaled overlays follow theme changes together. Keep overlay chroma restrained
and use theme text for neutral menu highlights, not a fixed blue fill.

## App icon

The application mark is an ivory G with a play-shaped cutout on a violet tile.
Its source is `public/app-icon.svg`. Use `bun run icons` to regenerate native
assets. The SVG uses fixed sRGB equivalents for native renderer compatibility.
Keep the transparent outer margin, rounded tile, and bold silhouette. Do not
add small labels, extra music symbols, or theme-specific variants.

## Window composition

Use the shared utilities in `src/styles.css` for all standard windows.

| Utility                | Use                                           |
| ---------------------- | --------------------------------------------- |
| `window-shell`         | Root geometry for a full window.              |
| `window-surface`       | Standard translucent window background.       |
| `window-drag-region`   | Tauri title-bar drag target.                  |
| `window-header`        | Standard 4 rem minimum header with a divider. |
| `window-kicker`        | Small uppercase section label.                |
| `window-title`         | Main window heading.                          |
| `window-copy`          | Supporting header copy.                       |
| `window-status`        | Compact neutral state label.                  |
| `window-panel`         | Standard grouped content surface.             |
| `window-panel-muted`   | Recessed log or progress surface.             |
| `window-alert-danger`  | Error message surface.                        |
| `window-alert-warning` | Warning message surface.                      |

Artwork is an immersive exception. It uses `window-shell` but keeps the artwork
across the full frame, native traffic lights, and one bottom scrim. Use compact
controls without cards or a decorative divider. Put the full-width seek slider
above auto-width time labels. Center transport above compact volume, favorite,
Open queue, and Open library actions. Use the accent for active shuffle and repeat.
The mini player is compact and uses the shared panel treatment rather than a full
window surface.

Keep Artwork playback controls visible while its native window is active,
independently of cursor position. When inactive, fade metadata and scrim as the
cursor enters or leaves the frame. Track native cursor and frame bounds in physical pixels
from the shell, including the native titlebar, with DOM hover as a fallback.
Do not activate the window to detect hover. Stop tracking on unmount and ignore
late samples. Hidden or minimized windows do not count as hovered.

Playback controls require native focus. Hover, pointer presses, slider drags,
and menus must not bypass this gate. Hidden playback controls are inert and
excluded from accessibility; Tab must not remove inert while inactive.
Slide the complete overlay down by the control region's reserved height on blur,
moving metadata into the freed space and controls below the frame. Reverse this
motion on activation. Keep layout geometry fixed; animate opacity and CSS translate,
not height or margins. Use 300 ms overlay entrances and 1200 ms exits with a smooth ease-out.
Keep metadata fade timing independent of control slide timing so inactive hover
still fades in at 300 ms. Crossfade thin bottom progress rather than popping it
in and out. Give buttons restrained hover and press feedback; keep slider positions
directly attached to the pointer. Reduced-motion mode skips motion. Native buttons
remain unchanged.

Bind the artwork context menu to the full-frame native drag surface behind the
controls. Do not hold controls open for native window drags. Provide the
same favorite and window actions as visible buttons. Seek drags show a timestamp
preview and commit once. Cancel the preview on track change, cancellation, or blur.
Disable seeking for unknown or zero duration. Playback state and window navigation
remain parent-owned. Keep the seek thumb mounted through completed playback
commands so repeated keyboard seeks retain focus. Expose formatted time on the
slider thumb for assistive technology.

Import uses one continuous window surface with compact link entry before search.
Reserve the native title-bar area above the heading. Keep import activity and
Cancel in a fixed footer outside the form scroll area. Use a bounded log viewport
and show the current phase rather than a second generic status badge. Do not show
an empty terminal before the first import.

Metadata refresh details open in a non-modal popover anchored to the status
button beside the library summary. Keep the library geometry unchanged. Use an
opaque popover surface and a bounded scroll area, with active work first and
completed jobs collapsed. Keep failures visible with a retry action. Closing the
popover must not stop background work.

## Type and spacing

Use the system sans font. Keep primary headings at 1.5 rem with a tight negative
tracking value. Use small, uppercase kickers for labels. Use muted and subtle
text for metadata before reducing font size further.

Use 1.5 rem horizontal header padding for full windows. Use 20 px panel padding.
Keep list rows compact and use dividers or soft selection fills instead of
independent cards.

## Interaction

Use `--accent` for the current selection, current playback, and primary actions.
Keep inactive icons and controls in `--subtle-text` or `--muted-text`. Hover
states use a low-opacity surface fill. Focus remains visible through
`--focus-ring`.

Use 150 to 250 ms ease-out transitions only for state changes, except Artwork's
coordinated 300 ms overlay entrances and 1200 ms exits. Respect the
global reduced-motion rule. Do not animate layout for decoration.

## Component boundaries

Use `Button`, `Slider`, `ScrollArea`, and other controls from
`src/components/ui` before creating a local version. Add a shared token or
utility only when at least three screens need the same semantic treatment. Keep
media artwork treatments local to artwork and playback components.

## Window checklist

- Use the shared shell, surface, header, panel, and alert utilities when they
  fit the task.
- Use semantic color variables instead of a hard-coded component color.
- Keep titles, supporting copy, states, and focus treatment consistent with
  Library.
- Preserve keyboard behavior, Tauri drag regions, and reduced-motion support.
- Keep artwork and mini-player surfaces distinct when their playback purpose
  needs it.
