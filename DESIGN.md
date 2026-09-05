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
as its surface. The mini player is compact and uses the shared panel treatment
rather than a full window surface.

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

Use 150 to 250 ms ease-out transitions only for state changes. Respect the
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
