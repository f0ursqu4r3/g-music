# G Music Design Guide

## Source

The Library window is the reference surface. It balances a dense collection view with a calm playback environment. Other windows use the same tokens and shared window utilities. They can change layout when their task needs it.

## Scene and register

G Music is a product interface for someone listening while they work, often in a low-light desktop environment. The interface uses dark, tinted surfaces so music controls remain visible without pulling focus from the primary task.

Use the product register. Favor familiar controls, compact spacing, and clear active states. Do not add marketing copy, decorative cards, or unrelated visual effects.

## Color

Use the semantic variables in `src/styles.css`.

| Role            | Variable                         | Use                                                   |
| --------------- | -------------------------------- | ----------------------------------------------------- |
| Canvas          | `--canvas`                       | Immersive artwork surface only.                       |
| Window surface  | `--glass-window`                 | Standard window background.                           |
| Panel           | `--surface`                      | Grouped content that needs separation.                |
| Control surface | `--glass-control`                | Inputs and compact secondary controls.                |
| Divider         | `--line`                         | Standard borders and section rules.                   |
| Strong divider  | `--line-strong`                  | Inputs and emphasized boundaries.                     |
| Primary text    | `--text`                         | Titles and selected content.                          |
| Supporting text | `--muted-text`                   | Metadata and explanatory copy.                        |
| Quiet text      | `--subtle-text`                  | Labels, counts, and inactive icons.                   |
| Accent          | `--accent`                       | Primary action, active selection, and playback state. |
| Accent wash     | `--accent-soft`                  | Selected rows and low-emphasis active state.          |
| Danger          | `--danger` and `--danger-soft`   | Errors only.                                          |
| Warning         | `--warning` and `--warning-soft` | Risk and caution only.                                |

Do not introduce raw color values in window components when a semantic variable fits. Themes redefine the color primitives and keep their semantic roles stable.

## Window composition

Use the shared utilities in `src/styles.css` for all standard windows.

| Utility                | Use                                           |
| ---------------------- | --------------------------------------------- |
| `window-shell`         | Root geometry for a full window.              |
| `window-surface`       | Standard translucent window background.       |
| `window-drag-region`   | Tauri title-bar drag target.                  |
| `window-header`        | Standard 88 px content header with a divider. |
| `window-kicker`        | Small uppercase section label.                |
| `window-title`         | Main window heading.                          |
| `window-copy`          | Supporting header copy.                       |
| `window-status`        | Compact neutral state label.                  |
| `window-panel`         | Standard grouped content surface.             |
| `window-panel-muted`   | Recessed log or progress surface.             |
| `window-alert-danger`  | Error message surface.                        |
| `window-alert-warning` | Warning message surface.                      |

Artwork is an immersive exception. It uses `window-shell` but keeps the artwork as its surface. The mini player is compact and uses the shared panel treatment rather than a full window surface.

## Type and spacing

Use the system sans font. Keep primary headings at 1.5 rem with a tight negative tracking value. Use small, uppercase kickers for labels. Use muted and subtle text for metadata before reducing font size further.

Use 32 px horizontal header padding for full windows. Use 20 px panel padding. Keep list rows compact and use dividers or soft selection fills instead of independent cards.

## Interaction

Use `--accent` for the current selection, current playback, and primary actions. Keep inactive icons and controls in `--subtle-text` or `--muted-text`. Hover states use a low-opacity surface fill. Focus remains visible through `--focus-ring`.

Use 150 to 250 ms ease-out transitions only for state changes. Respect the global reduced-motion rule. Do not animate layout for decoration.

## Component boundaries

Use `Button`, `Slider`, `ScrollArea`, and other controls from `src/components/ui` before creating a local version. Add a shared token or utility only when at least three screens need the same semantic treatment. Keep media artwork treatments local to artwork and playback components.

## Window checklist

- Use the shared shell, surface, header, panel, and alert utilities when they fit the task.
- Use semantic color variables instead of a hard-coded component color.
- Keep titles, supporting copy, states, and focus treatment consistent with Library.
- Preserve keyboard behavior, Tauri drag regions, and reduced-motion support.
- Keep artwork and mini-player surfaces distinct when their playback purpose needs it.
