# Symbol Audio — Style Reference
> listening room after dark, one red light on the amplifier

**Theme:** dark

Source measurements are normalized; roles and recommendations are interpreted. Font summary lists are independent, not paired by position. HTML examples are reconstructions, not source components.

Symbol Audio is a gray-black listening room: a graphite wall behind museum-label typography, with a single cinnabar red used the way a hi-fi uses its one indicator light. Graphite (#161618) is the canvas, brand and frame of every section. Raised surfaces are only a step lighter (charcoal), so the interface reads as one continuous dark wall. Red is rare and meaningful: it marks what is playing, the progress of the music and the one primary control. Two custom serifs carry all copy, and a sharp geometric sans handles headings — the contrast between rounded serif warmth and cold geometric boldness IS the voice. A single bottom marquee runs continuously.

## Tokens — Colors

The palette is two hues only: gray-black and red. Everything else is a tint of them.

| Name | Value | Token | Role |
|------|-------|-------|------|
| Graphite | `#161618` | `--color-graphite` | Canvas for every section, nav, sidebar, player bar and marquee. The primary color of the system |
| Charcoal | `#1f1f23` | `--color-charcoal` | Raised surface one step above the canvas: content cards, inputs, menus |
| Smoke | `#34343a` | `--color-smoke` | Hairline borders, dividers, card outlines, ghost-button borders, hover fills |
| Ash | `#e8e6e3` | `--color-ash` | Text and icons on graphite — warm light gray, never pure #fff. Secondary text uses Ash at 55–70% opacity |
| Cinnabar Red | `#d94a3d` | `--color-cinnabar` | The single accent: playing state, progress fill, active toggles, the primary play control, errors. Tints (12–35% opacity) for selected/playing row backgrounds |

## Tokens — Typography

### Chalet-LondonSixty — Primary body and UI typeface — all nav links, product names, prices, badge text, footer copy, button labels. A custom rounded serif with distinctive ball terminals; its single weight is used universally · `--font-chalet-londonsixty`
- **Substitute:** Recoleta, Domaine Display, GT Sectra, or Fraunces (SOFT 100) at weight 400
- **Weights:** 400
- **Sizes:** 11px, 13px, 15px, 16px
- **Line height:** 1.00–1.50
- **Letter spacing:** normal

### Chalet-NewYorkSixty — Secondary serif variant for small body and link micro-copy · `--font-chalet-newyorksixty`
- **Substitute:** Recoleta Regular or GT Sectra Regular
- **Weights:** 400
- **Sizes:** 11px, 13px, 15px
- **Line height:** 1.00, 1.33
- **Letter spacing:** normal

### SupremeLL-Bold — Display and heading typeface — section headings, the wordmark, and large UI moments. A sharp geometric sans that cuts against the serif body type. Large sizes with -0.02em tracking are the signature move · `--font-supremell-bold`
- **Substitute:** Inter Tight Bold, Söhne Breit Halbfett, or Neue Haas Grotesk Bold
- **Weights:** 400
- **Sizes:** 14px, 20px, 34px, 52px, 80px
- **Line height:** 1.00–1.33
- **Letter spacing:** -0.02em (tighter at larger sizes)

### Type Scale

| Role | Family | Weight | Size | Line Height | Letter Spacing | Token |
|------|--------|--------|------|-------------|----------------|-------|
| caption | — | — | 11px | 1.33 | 0px | `--text-caption` |
| body | — | — | 15px | 1.5 | — | `--text-body` |
| subheading | — | — | 20px | 1.33 | — | `--text-subheading` |
| heading-sm | — | — | 34px | 1.25 | -0.68px | `--text-heading-sm` |
| heading | — | — | 52px | 1.13 | -1.04px | `--text-heading` |
| display | — | — | 80px | 1 | -1.6px | `--text-display` |

## Tokens — Spacing & Shapes

**Base unit:** 4px · **Density:** comfortable

| Name | Value | Token |
|------|-------|-------|
| 4 | 4px | `--spacing-4` |
| 8 | 8px | `--spacing-8` |
| 16 | 16px | `--spacing-16` |
| 20 | 20px | `--spacing-20` |
| 40 | 40px | `--spacing-40` |
| 100 | 100px | `--spacing-100` |
| 200 | 200px | `--spacing-200` |

### Border Radius

| Element | Value |
|---------|-------|
| tags | 9999px |
| cards | 0px |
| badges | 9999px |
| buttons | 9999px |

### Layout

- **Section gap:** 40px
- **Card padding:** 20px
- **Element gap:** 20px

## Components

### Top Navigation / Sidebar
Graphite, no fill change, no border except a Smoke hairline where it meets the content. Wordmark in SupremeLL-Bold ~20px, Ash. Links in Chalet-LondonSixty 15px, Ash at 80%; hover brings them to full Ash with an underline. The selected item is an outlined pill (1px Smoke). The item that is playing shows its marker in Cinnabar Red.

### Announcement Marquee
Fixed to the viewport bottom, full-width band on Graphite. Continuously scrolling text in Chalet-LondonSixty 13px, Ash. Never scrolls away.

### Section Heading
SupremeLL-Bold at 34–52px, Ash, tracking -0.02em, left-aligned, trailing ellipsis ("All tracks…") is a signature punctuation choice. Sits directly on Graphite with no container.

### Content Card (placard)
Charcoal (#1f1f23) rectangle, 0px radius, 1px Smoke border, no shadow. Rows separated by Smoke hairlines. Primary text Ash, secondary Ash at 60%. Hover row: Smoke fill. Playing row: Cinnabar Red at ~14% fill with a red marker.

### Ghost Text Link
Chalet-LondonSixty 15px, Ash. No underline by default; underline on hover. No background, no border.

### Outlined Action Button
Transparent fill, 1px Smoke border, 9999px radius, 5–15px vertical / 14–20px horizontal padding. Label Chalet-LondonSixty 14–15px, Ash. Hover: border and label go to Ash with a faint Smoke fill. Never a filled solid button.

### Primary Control
The one control that matters most on a screen (play/pause) is an outlined circle in Cinnabar Red with a red glyph. Hover adds a 15% red tint. Only one red control per view.

### Progress / Level Bar
2px Smoke track, Cinnabar Red fill for progress, Ash knob (6px) on hover or drag.

## Do's and Don'ts

### Do
- Use Graphite as the canvas for every band — section transitions are invisible.
- Pair the Chalet serif for all body, nav, and labels with SupremeLL-Bold for every heading — never mix roles.
- Raise content onto Charcoal placards with a Smoke hairline and 0px radius.
- Use 9999px radius for every badge, tag, and pill button — sharp corners everywhere else.
- Reserve Cinnabar Red for state and the primary control: playing, progress, active toggles, errors.
- Keep the marquee fixed to the bottom edge on Graphite.
- Keep Ash text on Graphite/Charcoal above 12:1 contrast.

### Don't
- Do not introduce any hue other than gray-black and Cinnabar Red.
- Do not use red as a large fill or background — tints at 35% opacity at most.
- Do not introduce a filled solid-color button.
- Do not apply border-radius to cards or images.
- Do not set body text in SupremeLL-Bold.
- Do not add box-shadow or drop-shadow — surfaces are flat, contrast comes from tone.
- Do not use pure #ffffff or #000000 — the system runs Ash and Graphite at its extremes.

## Surfaces

| Level | Name | Value | Purpose |
|-------|------|-------|---------|
| 0 | Graphite Canvas | `#161618` | Full-window background — nav, sidebar, content, player bar and marquee |
| 1 | Charcoal Card | `#1f1f23` | Content placards, inputs, popup menus |
| 2 | Smoke | `#34343a` | Hover fill and hairlines only, never a large surface |

## Imagery

Product and cover imagery floats on Graphite with no frame, no rounded corners and no shadow. The app mark is a graphite record seen from above with Ash grooves and a Cinnabar Red label.

## Agent Prompt Guide

**Quick Color Reference**
- background: #161618 (graphite)
- card surface: #1f1f23 (charcoal)
- border / hover: #34343a (smoke)
- text: #e8e6e3 (ash)
- accent / primary action: #d94a3d (cinnabar red)

## Quick Start

### CSS Custom Properties

```css
:root {
  /* Colors */
  --color-graphite: #161618;
  --color-charcoal: #1f1f23;
  --color-smoke: #34343a;
  --color-ash: #e8e6e3;
  --color-cinnabar: #d94a3d;

  /* Typography — Font Families */
  --font-chalet-londonsixty: 'Chalet-LondonSixty', ui-serif, Georgia, serif;
  --font-chalet-newyorksixty: 'Chalet-NewYorkSixty', ui-serif, Georgia, serif;
  --font-supremell-bold: 'SupremeLL-Bold', ui-sans-serif, system-ui, sans-serif;

  /* Typography — Scale */
  --text-caption: 11px;
  --text-body: 15px;
  --text-subheading: 20px;
  --text-heading-sm: 34px;
  --text-heading: 52px;
  --text-display: 80px;

  /* Spacing */
  --spacing-4: 4px;
  --spacing-8: 8px;
  --spacing-16: 16px;
  --spacing-20: 20px;
  --spacing-40: 40px;

  /* Border Radius */
  --radius-cards: 0px;
  --radius-full: 9999px;

  /* Surfaces */
  --surface-graphite-canvas: #161618;
  --surface-charcoal-card: #1f1f23;
}
```
