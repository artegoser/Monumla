---

name: Monumla
description: A living games club for finding opponents, playing, and keeping every result.
colors:
ground: "#202427"
surface: "#2a2e31"
surface-raised: "#32373b"
surface-hover: "#383f43"
foreground: "#e6e8e5"
muted: "#a7b0b2"
line: "#ffffff12"
accent-sage: "#b9d5a1"
accent-ink: "#21321a"
error: "#f2ada5"
field: "#22272a"
typography:
display:
fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
fontSize: "clamp(30px, 3vw, 42px)"
fontWeight: 580
lineHeight: 1.12
letterSpacing: "-0.035em"
headline:
fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
fontSize: "34px"
fontWeight: 550
lineHeight: 1.2
letterSpacing: "-0.02em"
title:
fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
fontSize: "22px"
fontWeight: 550
lineHeight: 1.25
letterSpacing: "-0.02em"
body:
fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
fontSize: "15px"
fontWeight: 400
lineHeight: 1.6
label:
fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
fontSize: "12px"
fontWeight: 550
lineHeight: 1.35
rounded:
input: "6px"
button: "7px"
item: "8px"
panel: "12px"
avatar-large: "16px"
spacing:
xs: "4px"
sm: "8px"
md: "12px"
lg: "20px"
xl: "25px"
page: "40px"
components:
button-primary:
backgroundColor: "{colors.accent-sage}"
textColor: "{colors.accent-ink}"
rounded: "{rounded.button}"
padding: "13px 20px"
typography: "{typography.label}"
button-primary-hover:
backgroundColor: "#cae5b3"
textColor: "{colors.accent-ink}"
rounded: "{rounded.button}"
button-secondary:
backgroundColor: "{colors.surface-raised}"
textColor: "{colors.foreground}"
rounded: "{rounded.button}"
padding: "13px 20px"
typography: "{typography.label}"
input-field:
backgroundColor: "{colors.field}"
textColor: "{colors.foreground}"
rounded: "{rounded.input}"
padding: "11px 12px"
surface-card:
backgroundColor: "{colors.surface}"
rounded: "{rounded.panel}"
padding: "25px"

# Design System: Monumla

## Overview

**Creative North Star: "The Living Games Club"**

Monumla is a quiet, practical games club: finding an opponent is the first task, and the interface keeps the boards, clocks, people, and personal record close at hand. Graphite surfaces create a calm room for play while sage actions mark the next useful move. Clear system UI type and board-inspired rows keep the product legible across six games without turning the shell into a theme park.

The visual language is dense where decisions happen and generous around the playing space. Tonal surfaces, thin dividers, and small status signals establish hierarchy without decorative glow. Desktop uses a broad club layout; mobile keeps the same priorities by stacking the workbench and making horizontally clipped data discoverable.

**Key Characteristics:**

- Graphite ground with restrained tonal surface steps.
- Sage is reserved for actions, selection, links, and active state.
- System UI typography with a compact label scale and 30–42px operative page titles.
- Rounded practical controls and 12px sheets, with no ornamental effects.

## Colors

The palette is cool graphite and near-white text, with one soft sage action color and a warm coral error color. Contrast comes from tonal steps more than borders; the line color is a quiet translucent white.

### Primary

- **Soft Sage Action** (`{colors.accent-sage}`): The primary action, selected state, active navigation mark, caret, and live presence signal.

### Neutral

- **Graphite Ground** (`{colors.ground}`): The page background and the base around every playing surface.
- **Graphite Surface** (`{colors.surface}`): Pairing sheets, profile tables, match sidebars, and other primary containers.
- **Raised Graphite** (`{colors.surface-raised}`): Secondary buttons, time-control choices, and controls that need a step above a sheet.
- **Hover Graphite** (`{colors.surface-hover}`): Hover response for raised controls.
- **Foreground** (`{colors.foreground}`): Primary text and important player or game labels.
- **Muted Slate** (`{colors.muted}`): Supporting copy, metadata, empty-state guidance, and inactive navigation.
- **Translucent Divider** (`{colors.line}`): Hairline separators and table/list rhythm.
- **Sage Ink** (`{colors.accent-ink}`): Text placed on sage actions and active clocks.
- **Field Graphite** (`{colors.field}`): Input and select backgrounds.
- **Warm Error** (`{colors.error}`): Errors and destructive/resignation messaging.

### Named Rules

**The One Accent Rule.** Sage carries action and attention; do not introduce another competing accent for ordinary interface state.

## Typography

**Display Font:** System UI (`-apple-system`, BlinkMacSystemFont, `Segoe UI`, sans-serif)
**Body Font:** The same system UI stack
**Label/Mono Font:** Labels remain system UI; board notation and FEN/PGN fields use the platform monospace fallback where needed.

**Character:** The system stack is direct and familiar, with modest negative tracking on large titles and tabular numerals for ratings, clocks, and counts. Weight changes are restrained rather than theatrical.

### Hierarchy

- **Display** (580, `clamp(30px, 3vw, 42px)`, 1.12): Operative page titles such as “Find your next game.”
- **Headline** (550, 34px, 1.2): Profile identity and other section-leading names.
- **Title** (550, 22px, 1.25): Selected game headings and prominent sheet titles.
- **Body** (400, 15px, 1.6): Explanatory copy and page-level supporting text.
- **Label** (550, 12px, 1.35): Tabs, metadata, control labels, and compact navigation.

### Named Rules

**The Clear System Type Rule.** Keep the system UI stack, modest tracking, and tabular numerals; hierarchy comes from size, weight, and spacing rather than display-face switching.

## Layout

The main platform container is capped at 1300px with 40px desktop side padding. The lobby is a two-part club view: a game picker beside the pairing sheet, with a community rail alongside it. The workbench uses a 220px picker and a flexible sheet with a 20px gap; the lobby rail is 246px with a 36px column gap. Match pages use a board column beside a 260–340px sidebar.

At intermediate widths, the container tightens to 25px padding and the picker narrows. Below 900px, the lobby rail moves below the workbench. Below 700px, match pages become one column; below 620px, the header wraps navigation beneath the brand and account controls, and the ratings table advertises horizontal scrolling. Mobile keeps 12–18px edge padding and stacks the interaction path instead of hiding it.

## Elevation & Depth

The default depth model is tonal: ground, surface, raised, and hover steps do most of the work, with translucent dividers only where a list or sheet needs a boundary. The board frame and a few legacy analysis affordances use restrained functional shadows for separation; these are not ambient decoration. Do not add glows or floating effects to make ordinary controls feel important.

### Named Rules

**The Tonal Depth Rule.** Establish hierarchy with the graphite ramp first; use a shadow only when a board, overlay, or functional sheet must separate from its immediate surface.

## Shapes

Controls use gently rounded 6–8px corners: fields are 6px, buttons and game-picker items are 7–8px, and primary sheets are 12px. Large avatars use a softer 16px radius while compact avatars use 7px. Borders are quiet one-pixel translucent lines, not thick outlines; active navigation and selected controls use a 2px sage indicator or border.

## Components

### Buttons

- **Shape:** Practical rounded rectangle (7px), 44px minimum height for regular buttons.
- **Primary:** Soft sage background with sage-ink text, 13px 20px padding, semibold system label.
- **Hover / Focus:** Primary shifts to the lighter sage hover; secondary shifts to hover graphite. Keyboard focus uses a 2px sage outline with a 3px offset.
- **Secondary / Ghost / Tertiary:** Raised graphite for secondary actions; text buttons are unfilled sage links with compact vertical padding.

### Cards / Containers

- **Corner Style:** 12px for pairing sheets, form sheets, and match sidebars; 8–10px for smaller search or banner surfaces.
- **Background:** Graphite surface for primary containers; raised graphite for controls inside them.
- **Shadow Strategy:** Tonal layering by default; keep functional board/overlay shadows restrained.
- **Border:** Use translucent hairlines between sheet sections and list rows.
- **Internal Padding:** 20–25px for sheet content, with 40px page gutters on desktop.

### Inputs / Fields

- **Style:** Field graphite background, muted slate border, 6px radius, and 11px 12px internal padding.
- **Focus:** A 2px sage outline with a 3px offset, shared by inputs, selects, links, and buttons.
- **Error / Disabled:** Warm coral for error copy; disabled controls reduce opacity and use a not-allowed cursor.

### Navigation

- **Style:** Compact system labels in a horizontal header. Inactive links are muted; active links are foreground with a 2px sage underline. At mobile widths navigation wraps into a full-width row beneath the brand.

### Game Picker and Pairing Sheet

The lobby's signature workbench pairs six vertically stacked game choices with a tabbed pairing sheet. Chosen game rows use a translucent sage wash and sage icon/text; the sheet uses a 12px surface, 24–25px interior spacing, and raised time-control tiles. The primary play action spans the sheet width.

### Match Clock

Player strips keep identity and rating on the left and a large tabular clock on the right. The active clock uses sage background and sage ink; the inactive clock remains muted on the surface. The same state language carries through finished games and replay controls.

## Do's and Don'ts

### Do:

- **Do** use the graphite ground and tonal surface ramp to separate page, sheet, and control layers.
- **Do** reserve sage for actions, active navigation, selected game/time states, links, presence, and meaningful positive status.
- **Do** keep page titles within the observed 30–42px operative range and use system UI throughout.
- **Do** make mobile stacking and horizontally scrollable rating data explicit rather than silently clipping important content.
- **Do** preserve visible focus outlines and reduced-motion behavior for interactive controls.

### Don't:

- **Don't** add decorative glows, gradients, or a second bright accent to ordinary platform surfaces.
- **Don't** replace the system type with a display face or turn labels into ornamental eyebrows.
- **Don't** use thick borders or hard offset shadows as a default surface treatment.
- **Don't** hide game choice, pairing, clock, or rating information when the layout becomes narrow.
