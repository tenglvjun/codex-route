# Design System: Payflux Fast Payments

## 1. Visual Theme & Atmosphere

Payflux is a calm, trust-first fintech SaaS interface: airy like a well-lit finance studio, precise enough for money movement, and friendly without becoming playful. The page uses a cool mist canvas, quiet white surfaces, charcoal type, and one confident blue accent. Large whitespace around the hero gives the product room to breathe; the dashboard preview below supplies density and proof.

- **Density:** Daily App Balanced — 4/10. Keep the marketing surface spacious; allow the dashboard to become information-rich without feeling crowded.
- **Variance:** Predictable Symmetric — 3/10. The centered hero and stable navigation are intentional. Create interest through scale, rhythm, and the dashboard's internal asymmetry rather than random offsets.
- **Motion:** Fluid CSS — 4/10. Interaction should feel weighty and polished, never theatrical or distracting from financial tasks.
- **Design keywords:** lucid, dependable, modern, rounded, quiet confidence, operational clarity.
- **Brand promise:** Make payments feel simpler, faster, and more legible. Prefer plain-language explanations over fintech jargon.
- **Surface rule:** Use a single cool-neutral family from the page canvas through the dashboard. Do not mix warm grays with cool grays.

## 2. Color Palette & Roles

The palette is intentionally restrained. Payflux Blue is the only chromatic accent; its lighter tint is a supporting surface wash, not a second accent. Never use a gradient to create hierarchy.

- **Mist Canvas** (`#F4F7FF`) — primary page background; sampled from the hero field. Use edge-to-edge behind marketing sections.
- **Pure Surface** (`#FFFFFF`) — navigation, cards, dashboard panels, input fields, and primary content containers.
- **Soft Utility Surface** (`#FAFAFA`) — recessed controls, chart wells, hover fills, and skeleton bases when white-on-mist contrast is insufficient.
- **Charcoal Ink** (`#1E2020`) — headings, primary body copy, icons, and the dark primary CTA. This replaces pure black.
- **Muted Steel** (`#616365`) — supporting copy, metadata, chart labels, inactive navigation, and helper text.
- **Whisper Border** (`#EAEAEA`) — 1px separators, card outlines, input boundaries, and dashboard chrome. Keep borders low-contrast and structural.
- **Payflux Blue** (`#2158DC`) — the single accent for logo marks, active navigation, focus rings, chart emphasis, progress indicators, and selected states.
- **Blue Wash** (`#E1E9FF`) — a low-saturation tint of Payflux Blue for the hero badge, selected backgrounds, and non-text callouts. Do not use it for large gradients.

### Contrast and state guidance

- Use Charcoal Ink on Mist Canvas or Pure Surface for primary text.
- Use Payflux Blue on Pure Surface for controls and on Blue Wash only when the label remains clearly legible.
- Disabled controls use Muted Steel at reduced opacity plus Soft Utility Surface; do not introduce a new gray family.
- Success, warning, and error states should be communicated with a short label, icon, and border treatment first. If a semantic hue is unavoidable, use a muted, desaturated tone and keep Payflux Blue as the dominant brand signal.

## 3. Typography Rules

Use a geometric grotesk with rounded terminals to echo the reference's friendly precision. All product surfaces remain sans-serif; numbers may switch to a mono face for scanning.

- **Display / headlines:** `Satoshi`, fallback `ui-sans-serif, system-ui, sans-serif`. Weight 600–700, tracking `-0.045em` to `-0.02em`, tight leading between `0.98` and `1.08`.
- **Body / navigation:** `Satoshi`, weight 400–500, tracking `-0.01em`, leading `1.45`–`1.6`. Keep paragraphs to approximately 65 characters per line.
- **Numbers / transaction IDs:** `Geist Mono`, fallback `ui-monospace, SFMono-Regular, monospace`. Use tabular figures and a slightly looser `0.01em` tracking for balances and timestamps.
- **Hero headline:** 72px desktop (`clamp(3rem, 6.1vw, 5.5rem)`), max width 860px, two deliberate lines, never an uncontrolled wall of text. On mobile use `clamp(2.5rem, 13vw, 4rem)` and preserve a readable two-to-four-line rhythm.
- **Section headings:** 40–48px desktop, 32–36px mobile; weight 600; use sentence case.
- **Card headings:** 20–24px, weight 600. Metric values: 24–32px, weight 600, Geist Mono where values need rapid comparison.
- **Labels / navigation:** 15–16px, weight 500. Metadata and chart axes: 12–14px, weight 400–500, Muted Steel.
- **Microcopy:** never smaller than 12px for essential information; use 14px as the default minimum for body UI.
- **Banned type:** `Inter` and generic system-only typography for premium surfaces; generic serif faces (`Times New Roman`, `Georgia`, `Garamond`, `Palatino`) are not part of this system.

## 4. Shape, Spacing & Elevation Tokens

- **Base spacing unit:** 8px. Prefer values from the sequence 8, 12, 16, 24, 32, 40, 48, 64, 80, and 96px.
- **Page containment:** cap main content at 1440px with 32–64px horizontal gutters on desktop and 20px on mobile.
- **Hero rhythm:** 32px top navigation inset; 80–112px from nav to badge; 24px badge-to-heading; 24–32px heading-to-description; 40px description-to-CTA.
- **Radii:** pill controls `9999px`; buttons and badges fully pill-shaped; cards 24px; dashboard shell 28–32px; compact controls 12–16px.
- **Borders:** 1px Whisper Border. Do not stack multiple visible borders around one element.
- **Shadows:** use one diffused, cool-neutral shadow only where elevation is meaningful: `0 12px 40px rgba(30, 32, 32, 0.08)`. Avoid hard black drop shadows and colored glows.
- **Focus ring:** 2px Payflux Blue with a 2px transparent offset; preserve a visible focus state on every keyboard-accessible control.

## 5. Component Stylings

### Navigation

- Place the Payflux wordmark at the left with generous clear space; do not redraw or distort the logo.
- Desktop navigation is a single horizontal row: Company, Services, Pricing, Testimonial, Support. Use 16px labels, 32–40px gaps, and Charcoal Ink.
- Keep Sign Up as a light pill on Pure Surface and Login as a Charcoal Ink pill. Both are at least 48px tall with 24px horizontal padding.
- Below 768px, collapse links into a clean menu button. Keep the Login action visible if space permits; never allow navigation to wrap into two rows.

### Hero badge

- Use a small Blue Wash pill (approximately 248px × 48px on desktop) with a thin circular dollar icon and the label “Payflux Fast Payments”.
- Icon and text use Payflux Blue. The badge is a context cue, not a competing CTA.

### Buttons

- **Primary:** Charcoal Ink fill, white label, 52–60px height, 9999px radius, 16px horizontal padding minimum 28px. Label examples: “Get started”.
- **Secondary:** Pure Surface fill with Whisper Border or very subtle tonal contrast, Charcoal Ink label. Label example: “Book a demo”.
- Hover transitions may shift the button up 2px and deepen the shadow slightly; active state translates down 1px to feel tactile.
- Keep one primary CTA per section. Never use neon outlines, gradient fills, or icon-only primary actions.

### Dashboard shell

- Present the product preview as a large Pure Surface shell with a 28–32px radius, a 1px Whisper Border, and a low diffused shadow.
- Use a stable left rail, a top utility row, and a two-column content area. The rail carries the blue active indicator; inactive icons stay Muted Steel.
- Toolbar controls (search, date range, Add Widget) share 48–52px heights, 12–16px radii, and consistent internal padding.
- Keep the shell's internal spacing on the 8px grid. White panels sit on the Mist Canvas or Soft Utility Surface to establish hierarchy without heavy borders.

### Metric cards and charts

- Balance, Amount Spent, and Amount Invested are compact metric modules, not three decorative marketing cards. Each pairs a circular progress glyph with a label and a Geist Mono value.
- Use Payflux Blue only for the active metric arc, selected tab underline, primary chart line, and progress fill; secondary arcs and chart scaffolding use Whisper Border or Muted Steel.
- Chart tabs use a 2px blue underline for the selected tab. Axis labels remain small and quiet. Never use pie-chart rainbow palettes or ornamental 3D effects.

### Cards, inputs and status

- Use cards only when elevation clarifies grouping. In dense lists prefer whitespace and border-top dividers over a wall of nested cards.
- Inputs place a label above the field, helper text below when needed, and inline error text beneath the helper. Do not use floating labels.
- Search fields use a subtle leading icon, 16px text, Pure Surface fill, and a visible Payflux Blue focus ring.
- Loading states are layout-matched skeleton blocks with a quiet shimmer. Do not use generic circular spinners.
- Empty states combine a concise explanation with a useful next action and a composed illustration or icon; never show only “No data”.
- Error states are inline, specific, and recoverable (“Couldn’t load transactions — Retry”), with a neutral border treatment and accessible text.

## 6. Layout & Responsive Principles

- Build with CSS Grid and explicit tracks; avoid percentage-based `calc()` hacks and absolute-positioned content that can collide.
- The marketing hero is centered and intentionally symmetrical at this low-variance setting. Every text block occupies its own clean zone; nothing overlaps the dashboard preview.
- Use a max-width container, then let the dashboard shell extend visually toward the viewport edges while retaining safe gutters.
- Do not default to a generic row of three equal feature cards. For supporting content use a two-column zig-zag, a primary-plus-rail composition, or a horizontal scroller with clear hierarchy.
- **Mobile-first breakpoint:** below 768px, all multi-column content becomes one column. Preserve the order: badge → headline → description → primary CTA → secondary CTA → dashboard preview.
- Prevent horizontal overflow at every viewport. Dashboard tables may become stacked summaries or intentional, contained scrollers with visible affordance.
- Keep interactive targets at least 44×44px. Reduce section gaps proportionally with `clamp(3rem, 8vw, 6rem)` rather than abrupt jumps.
- Use `min-h-[100dvh]` for full-height sections; never rely on fixed `h-screen` behavior on mobile Safari.

## 7. Motion & Interaction

- Default to premium spring physics: `stiffness: 100`, `damping: 20`. If a CSS-only implementation is required, use a cubic-bezier that eases in and out rather than linear timing.
- Reveal hero elements in a short cascade: badge → headline → description → actions → dashboard, with 40–70ms stagger increments.
- Buttons, tabs, rail items, and metric cards respond with transform and opacity only. Hover lift is 2px maximum; avoid bouncing or elastic overshoot.
- Active dashboard indicators may use a restrained infinite pulse or shimmer, but perpetual motion must remain subtle and pause when `prefers-reduced-motion: reduce` is enabled.
- Animate exclusively `transform` and `opacity` for performance. Never animate `top`, `left`, `width`, or `height`; keep grain/noise effects off this product's clean fintech surfaces.

## 8. Content & Voice

- Write in direct, human English with short sentences and concrete verbs: “Get started”, “Book a demo”, “Add Widget”.
- Explain the outcome before the mechanism. Keep claims specific and verifiable; avoid inflated conversion language.
- Use sentence case for navigation and headings. Capitalize product names and financial terms consistently.
- Do not invent customer names, logos, transaction values, or performance percentages that could be mistaken for real evidence.

## 9. Anti-Patterns (NEVER DO)

- Never use emojis in navigation, marketing copy, empty states, or dashboard chrome.
- Never use `Inter`, generic serif fonts, or pure black (`#000000`).
- Never add neon accents, purple/blue glows, oversaturated secondary colors, or gradient text on large headlines.
- Never use overlapping layers, floating labels, or absolute-positioned text that can collide at other widths.
- Never ship a generic three-equal-card feature row, rainbow chart palette, ornamental 3D finance imagery, or hard drop shadows.
- Never create fake “99.99%” metrics, “John Doe”/“Acme” placeholders, fake testimonials, or broken remote image links.
- Never use AI copywriting clichés such as “Elevate”, “Seamless”, “Unleash”, “Next-Gen”, or “revolutionary”.
- Never add filler UI copy such as “Scroll to explore”, “Swipe down”, bouncing chevrons, or decorative scroll arrows.
- Never introduce a second visual language (warm gray surfaces, unrelated radii, alternate button shapes, or a competing accent) on another page.
