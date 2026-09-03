# Codex Route Desktop Design System

Codex Route is a local desktop utility for switching Codex providers and controlling a local route listener. Its interface is an operator workbench, not a marketing page. The visual language takes cues from cc-switch: a dark graphite window, a compact toolbar, quiet list rows, and a mint signal for the current state.

## 1. Product Read

- **Audience:** developers who repeatedly change a local provider or workspace route.
- **Character:** focused, native, compact, dependable, and easy to scan.
- **Density:** 7/10. Show the state and the next action without decorative whitespace.
- **Variance:** 2/10. Use a stable toolbar and a predictable provider list.
- **Motion:** 2/10. Prefer immediate, restrained feedback over promotional animation.
- **Primary workflow:** choose a provider, confirm the route state, then open a secondary panel only when configuration is needed.

Do not bring web landing-page patterns into the client. There is no hero, headline, promotional badge, marketing navigation, mobile rail, or full-page card grid.

## 2. Window & Shell

The application is rendered inside a Tauri window and must feel like a native utility window.

- Preserve the **native Tauri title bar** and macOS traffic-light controls. Do not draw replacement window controls in React.
- Design the primary viewport for **1024 x 720 px**. The compact supported viewport is **800 x 560 px**.
- The root shell uses a single graphite canvas and fills the viewport: `min-height: 100dvh; min-width: 800px` in the desktop build.
- Keep the content inside the window. Never use a full-bleed website container, page-level scrolling, or a centered hero.
- Use a 64px toolbar at the top, a one-line context strip when needed, and a scrollable provider list below.
- At 800px wide, toolbar actions may collapse into icon buttons, but the provider name, current-state indicator, and primary action remain visible.
- If the native window is resized below the supported width, preserve layout integrity with contained scrolling rather than allowing controls to overlap.

### Shell regions

1. **Toolbar:** product name, route switch, workspace scope, provider mode, utility actions, and the add action.
2. **Workspace frame:** the active provider list and its empty/loading/error states.
3. **Secondary panel:** import, workspace rules, and advanced settings opened on demand. It may be a side drawer or an anchored panel; it must not replace the provider list context.

Recommended structural hooks are `.client-window`, `.client-toolbar`, `.workspace-frame`, `.provider-list`, and `.secondary-panel`. Keep these names stable so visual states can be tested without coupling styles to component internals.

## 3. Color Tokens

Use one cool graphite family. Colors are opaque and deliberate; do not add gradients, glass blur, or glow effects.

| Token | Value | Role |
| --- | --- | --- |
| `--graphite-canvas` | `#1b1b1f` | Window background |
| `--graphite-toolbar` | `#202023` | Toolbar and titlebar-adjacent chrome |
| `--graphite-surface` | `#24252a` | Provider rows and secondary surfaces |
| `--graphite-raised` | `#2d2e33` | Hover, selected, and active control backgrounds |
| `--graphite-inset` | `#16171a` | Inset search fields and segmented controls |
| `--graphite-border` | `#3a3b42` | Structural 1px borders and dividers |
| `--ink-primary` | `#f1f3f1` | Headings, names, and primary labels |
| `--ink-secondary` | `#a2a5ac` | URLs, metadata, inactive icons, and helper text |
| `--ink-muted` | `#777982` | Timestamps and tertiary labels |
| `--mint-accent` | `#39d39f` | Active route, selected provider, focus, and positive confirmation |
| `--mint-wash` | `#173c33` | Active row and route-control background |
| `--danger-muted` | `#f28a8a` | Destructive or blocked feedback text |

Contrast requirements:

- `--ink-primary` on `--graphite-canvas` and `--graphite-surface` must meet WCAG AA.
- `--ink-secondary` is for supporting text only; never use it for a critical control label.
- Mint is paired with dark graphite for filled controls and with `--mint-wash` for selected backgrounds.
- A disabled control uses `--ink-muted`, reduced opacity, and no hover treatment. Do not invent additional gray or purple tokens.

## 4. Typography

- **UI sans:** `Satoshi`, then `-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`. Native system rendering is acceptable for utility chrome.
- **Values and local URLs:** `Geist Mono`, then `ui-monospace, SFMono-Regular, Menlo, monospace`.
- **Window/product name:** 18px, weight 650, tracking `-0.02em`.
- **Provider name:** 16px, weight 600, line-height 1.25.
- **URL and metadata:** 13px, weight 400, line-height 1.4; URLs use `--ink-secondary` and may wrap within a row.
- **Toolbar labels:** 13px, weight 550. Icon-only controls still require an accessible name and tooltip.
- **Status labels:** 12px, weight 650, sentence case. Use uppercase only for a short non-interactive section eyebrow.
- **Route values:** 12-13px mono with tabular figures for ports and process IDs.
- Never use `Inter`, decorative serif faces, oversized hero typography, or text scaled with viewport width.

## 5. Shape, Spacing & Elevation

- Base spacing unit: 4px. Use 4, 8, 12, 16, 20, 24, and 32px.
- Toolbar horizontal padding: 20px at 1024px; 16px at compact width.
- Provider list gap: 8px. Provider row padding: 14px 16px; minimum height 72px.
- Group radius: 16px. Provider row radius: 12px. Small controls: 10px. Toggle and status chips: 9999px.
- Use a single 1px `--graphite-border` around a grouped surface. Do not stack card borders.
- Elevation is subtle: `0 12px 30px rgba(0, 0, 0, 0.24)` for a drawer or raised menu only. Rows remain mostly flat.
- Focus ring: 2px `--mint-accent` with a 2px transparent offset. Keep it visible on keyboard navigation.
- Interactive targets are at least 40 x 40px; toolbar icon buttons are 44 x 44px when space allows.

## 6. Toolbar

The toolbar is the visual anchor. It is one horizontal control band, not a website navigation bar.

- Use a product mark and `Codex Route` at the left; keep the wordmark compact.
- Keep route state close to the toolbar as a switch/control: `.toolbar-route` with `.route-toggle` and `.route-toggle-label`.
- The route toggle exposes `aria-pressed` or `role="switch"`, a `data-route-state` value, and a visible mint indicator when active.
- The workspace scope is a compact select or menu with a folder icon. Do not use a large page heading for it.
- Provider mode and utility controls sit in grouped graphite surfaces with 8-12px internal gaps.
- The add action is a 44-48px mint circular button with a plus icon and an accessible label. It is the only prominent create control.
- Hide or disable controls according to capability; do not leave decorative toolbar icons that do nothing.
- Tooltips name unfamiliar icon-only actions. Tooltip text is supplementary, never the only label for a critical action.

## 7. Route Control Strip

`RouteStatusPanel` is a compact operational strip used below the toolbar or inside a secondary overview region. It must not read as a marketing card.

- Root class: `.route-control-strip` (keep `.route-status-card` as a compatibility class if existing CSS references it).
- Root and controls expose `data-route-state="loading|inactive|active|external-modified"`.
- The strip contains a short identity/status head, listener/configuration/URL details, an external-change warning when present, a port field, and Activate/Deactivate actions.
- Keep the active state visually stronger with mint text, a mint dot, and `--mint-wash`; inactive/loading states remain quiet.
- Keep the port input disabled while busy or active. Keep Activate disabled while busy, active, or when no provider can be activated. Keep Deactivate disabled while busy or inactive.
- The warning for external configuration changes is inline and recoverable. Do not hide it in a toast.
- The status text remains a live region (`role="status"`, `aria-live="polite"`). Alerts use `role="alert"`.
- Route URLs and ports use mono typography and must remain selectable/copyable. Do not truncate without an accessible full value.

## 8. Provider List

The provider list is the default work surface and should resemble a native list, not a grid of promotional cards.

- Render one row per provider in stable order. Rows use `--graphite-surface`, a 1px border, and a 12px radius.
- A row contains a drag/reorder affordance when supported, a 40px identity tile, provider name, URL, status/current marker, and row actions.
- The current provider has a mint border or inset indicator, a mint status dot, and a clear `Current` label. Do not rely on color alone.
- Inactive rows remain readable and use `--ink-secondary`; do not reduce opacity enough to hide the provider URL.
- Destructive row actions are icon buttons with tooltips and confirmation where data can be lost. Keep them visually quiet until hover/focus.
- Empty state: explain how to import or add a provider and expose one useful action. Never show only "No data".
- Loading state: use row-shaped skeletons matching the final height. Avoid a generic page spinner.
- Error state: show the failed operation and a Retry action in the list context.

## 9. Secondary Panels

- Import and workspace rules open in a secondary panel or drawer with a clear title, close action, and preserved provider-list context.
- Recommended width: 360-420px on the 1024px window; full-width only below the compact breakpoint.
- Use a raised graphite surface, 16px radius, and one divider between header, content, and footer actions.
- Forms use labels above fields, helper/error text below, 44-48px controls, and explicit save/cancel actions.
- Group related controls with spacing and dividers, not nested cards. Avoid modal stacks.
- Keep destructive actions separated from primary save/import actions and use `--danger-muted` for warning copy.

## 10. Interaction & Motion

- Default transition: 140-180ms ease-out for background, border, color, opacity, and transform.
- Hover rows lift at most 1px or change border/background; never bounce or glow.
- Active buttons press by `translateY(1px)` or `scale(0.98)` for tactile feedback.
- Route activation, provider selection, import completion, and errors update in place. Do not navigate to a new marketing-style page.
- Never animate layout geometry, use continuous decorative motion, or rely on color-only state changes.
- Respect `prefers-reduced-motion: reduce` by removing transforms and transitions.

## 11. Content & Accessibility

- Use direct action labels: `Activate`, `Deactivate`, `Import`, `Add provider`, `Refresh`, `Retry`.
- Explain local behavior plainly. Avoid claims, fake metrics, invented provider names, or marketing cliches.
- Every icon-only button has an accessible name. Every form control has a persistent label.
- Keyboard focus follows the same order as the visual workflow: toolbar, route control, provider list, secondary panel.
- Status is communicated with text plus an indicator, never color alone. Preserve selectable URLs and meaningful error text.

## 12. Banned Patterns

- No hero sections, marketing CTAs, testimonial blocks, large dashboard previews, or web-style mobile rails.
- No light mist canvas, Payflux-blue web palette, purple gradients, neon glows, glassmorphism, or decorative blobs.
- No replacement titlebar or custom traffic lights.
- No nested cards, three-column feature grids, rainbow charts, ornamental 3D imagery, or hard black shadows.
- No placeholder copy such as "John Doe", fake balances, fake performance percentages, or "Next-Gen" claims.
- No controls that appear interactive but do not invoke a real action.
