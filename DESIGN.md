
# DESIGN.md - Pikachu Color Palette Design System (PC Web) - Optimized Version
> **Theme**: Pikachu UI Design System
> **Target Platform**: PC Web / Desktop SaaS Dashboard Applications
> **Style Positioning**: Energetic Neo‑Brutalist SaaS UI (Not cartoon skin)
> **Core Brand Palette**: `#f4dc26` (Electric Yellow), `#ffe62d` (Bright Flash), `#e92929` (Thunderbolt Red), `#5c3613` (Brown Fur/Accents), `#000000` (Electric Outline Black)

---

## 1. Design Philosophy & Token Architecture

The Pikachu Design System translates the iconic, energetic, and high‑contrast visual identity of Pokémon's mascot into a modern, highly usable PC‑first SaaS dashboard interface.
We use bold chromatic accents, high‑contrast sharp borders, playful yet clean typography, and crisp geometrical tokens.

**Important Design Constraint (Optimized Rule)**:
> Brand yellow is an accent color, NOT a background color. Limit large yellow surfaces.
> Red thunder color is reserved exclusively for danger, errors and high‑attention states.

Suitable scenarios: admin dashboards, project management tools, analytics workspace, community backend platforms.

---

## 2. Color System

### 2.1 Core Palette Mapping
| Token Name | Hex Code | Usage Description | Area Limitation Rule |
| :--- | :--- | :--- | :--- |
| **Primary (Brand / Main)** | `#f4dc26` | Main branding, primary action buttons, active tabs, key highlights. | Accent only, never use for full‑width backgrounds or large blocks (> 15% screen area) |
| **Primary Light (Flash)** | `#ffe62d` | Hover states, glowing effects, soft backgrounds, badges, chart highlight. | Small accent element only |
| **Secondary (Accent / Alert‑Danger)** | `#e92929` | Danger actions, notifications, critical badges, error alerts, focus‑ring. | Danger / error only. DO NOT use for normal decoration. |
| **Tertiary (Details / Muted)** | `#5c3613` | Deep contrast text, card headers, footer backgrounds, deep shadows, dashed borders. | Secondary text & decorative UI |
| **Monochrome Dark (Outline)** | `#000000` | Borders, primary body text, high‑contrast icons, structural dividers. | Main text & hard outline border |

### 2.2 Functional Colors
* **Background (Canvas)**: `#FFFDF4` (Warm off‑white paper tone for reduced eye strain on PC screens)
* **Surface (Cards / Modals)**: `#FFFFFF` (Pure white containers)
* **Foreground (Text Main)**: `#000000` (Maximum legibility)
* **Foreground Muted**: `#5c3613` (Secondary text, subtitles, captions)
* **Border Color‑Default Heavy**: `#000000` (Bold neo‑brutalism 2px border)
* **Border Color‑Subtle**: `#5c3613` (1px soft separation border)
* **Focus Ring**: `#e92929` with 3px spread (High visibility accessibility outline)

#### 2.3 Neutral Extended Scale (New Added for SaaS)
For table, divider, disabled states, form helper backgrounds
- `--neutral‑100: #f7f5ec`
- `--neutral‑200: #e5e5e5`
- `--neutral‑300: #cccccc`
- `--neutral‑400: #888888`

#### 2.4 Success & Info Functional Colors (Missing before, added for SaaS dashboard)
> Not part of pikachu brand palette, functional status only
- Success Green: `#229948`
- Info Blue: `#2469d8`
- Warning Orange: `#f29c1f`

#### 2.5 Accessibility Contrast Check
- Black text `#000000` on warm white canvas `#FFFDF4` → WCAG AA compliant
- Muted brown text `#5c3613` on white background → meets minimum readable contrast
- Never put pure black text on top of primary yellow `#f4dc26` for long paragraph text (only short button text)

---

## 3. Typography (H1‑H6, Body)

Using a clean modern sans‑serif stack (`Inter`, `PingFang SC`, `Microsoft YaHei`) combined with strong font weights to capture the energetic feel.

| Element | Font Size | Font Weight | Line Height | Letter Spacing | Color |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **H1** | `36px` (2.25rem) | `800` (ExtraBold) | `1.2` | `-0.02em` | `#000000` |
| **H2** | `28px` (1.75rem) | `700` (Bold) | `1.25` | `-0.01em` | `#000000` |
| **H3** | `22px` (1.375rem)| `700` (Bold) | `1.3` | `0` | `#000000` |
| **H4 (Card Header Title)** | `18px` (1.125rem)| `600` (SemiBold)| `1.4` | `0` | `#000000` |
| **H5** | `16px` (1.0rem) | `600` (SemiBold)| `1.4` | `0` | `#5c3613` |
| **H6** | `14px` (0.875rem)| `600` (SemiBold)| `1.5` | `0.01em` | `#5c3613` |
| **Body Large** | `16px` (1.0rem) | `400` (Regular)| `1.6` | `0` | `#000000` |
| **Body Base** | `14px` (0.875rem)| `400` (Regular)| `1.5` | `0` | `#000000` |
| **Caption / Small** | `12px` (0.75rem)| `400` (Regular)| `1.4` | `0.02em` | `#5c3613` |

* **Font Family Stack**:
font-family: 'Inter', system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;

---

## 4. Spacing & Border Radius Tokens

### 4.1 Spacing Scale (8pt Grid)
* `space‑2xs`: `4px`
* `space‑xs`: `8px`
* `space‑sm`: `12px`
* `space‑md`: `16px` (Default padding/margin)
* `space‑lg`: `24px`
* `space‑xl`: `32px`
* `space‑2xl`: `48px`
* `space‑3xl`: `64px`

### 4.2 Border Radius
* `radius‑none`: `0px` (Strict geometric style)
* `radius‑sm`: `4px` (Inputs, small buttons)
* `radius‑md`: `8px` (Standard cards, dropdowns, tabs)
* `radius‑lg`: `16px` (Modals, large containers)
* `radius‑xl`: `12px` (Dashboard large overview cards)
* `radius‑full`: `9999px` (Pills, circular badges, avatar icons)

---

## 5. Layout & Grid Architecture (PC Web)

* **Max Container Width**: `1280px` (Centered with auto margins)
* **Grid System**: 12‑Column Grid with `24px` gutter width.
* **Layout Structure (Standard Dashboard / Portal)**:
  * **Header**: Fixed top bar (`height: 64px`, background `#FFFFFF`, bottom border `2px solid #000000`).
  * **Sidebar**: Fixed or collapsible left sidebar (`width: 260px`, background `#FFFDF4`, right border `2px solid #000000`).
  * **Main Content Area**: Flexible workspace (`background: #FFFDF4`, padding `32px`).
  * **Cards & Panels**: Elevated surfaces (`background: #FFFFFF`, border `2px solid #000000`).

> ✨Optimized Shadow Rule (Major Change):
> Do NOT apply hard‑offset black shadow to EVERY card.
> - Primary important cards: `4px 4px 0px #000000` (--shadow‑neo)
> - Normal secondary cards: `2px 2px 0px #000000` (--shadow‑neo‑sm)
> - Static read‑only cards: No offset shadow, only border

---

## 6. Interactive States & Base Components

### 6.1 Buttons
All buttons feature high‑contrast borders and sharp or semi‑rounded corners (`radius‑md`) with solid drop shadows for tactile feedback.

#### A. Primary Button (`.btn‑primary`)
* **Default**:
  * Background: `#f4dc26`
  * Text: `#000000` (Font‑weight: `700`)
  * Border: `2px solid #000000`
  * Box‑shadow: `3px 3px 0px #000000`
* **Hover (`:hover`)**:
  * Background: `#ffe62d`
  * Transform: `translate(-1px, -1px)`
  * Box‑shadow: `4px 4px 0px #000000`
* **Active / Force (`:active`)**:
  * Background: `#f4dc26`
  * Transform: `translate(2px, 2px)`
  * Box‑shadow: `1px 1px 0px #000000`
* **Disabled (`:disabled`)**:
  * Background: `#E5E5E5`
  * Text: `#A3A3A3`
  * Border: `2px solid #CCCCCC`
  * Box‑shadow: `none`
  * Cursor: `not‑allowed`

#### B. Ghost Button (`.btn‑ghost`)
* **Default**:
  * Background: `transparent`
  * Text: `#000000` (Font‑weight: `600`)
  * Border: `2px solid transparent`
* **Hover (`:hover`)**:
  * Background: `rgba(244, 220, 38, 0.15)`
  * Border: `2px solid #000000`
* **Active / Force (`:active`)**:
  * Background: `rgba(244, 220, 38, 0.3)`
* **Disabled (`:disabled`)**:
  * Text: `#A3A3A3`
  * Border: `2px solid transparent`

#### C. Dashed Border Button (`.btn‑dashed`)
* **Default**:
  * Background: `#FFFFFF`
  * Text: `#5c3613` (Font‑weight: `600`)
  * Border: `2px dashed #5c3613`
* **Hover (`:hover`)**:
  * Background: `#FFFDF4`
  * Border: `2px dashed #000000`
  * Text: `#000000`
* **Active / Force (`:active`)**:
  * Background: `#f4dc26`
  * Border: `2px solid #000000`
* **Disabled (`:disabled`)**:
  * Background: `#FAFAFA`
  * Text: `#CCCCCC`
  * Border: `2px dashed #CCCCCC`

#### D. Danger Button (`.btn‑danger`) [NEW]
* Default: bg `#e92929`, text white, border `2px solid #000`
* Hover: `#f14c4c`

### 6.2 Form Inputs & Controls
* **Input Field (`input[type="text"]`)**:
  * Height: `40px`
  * Background: `#FFFFFF`
  * Border: `2px solid #000000`
  * Border‑radius: `6px`
  * Padding: `0 12px`
  * Font‑size: `14px`
  * **Focus State**: Outline none, border color `#e92929`, box‑shadow `0 0 0 3px rgba(233, 41, 41, 0.15)`.
  * **Disabled State**: Background `#F5F5F5`, border color `#CCCCCC`, text `#888888`.

### 6.3 Cards & Containers (`.card`)
* Background: `#FFFFFF`
* Border: `2px solid #000000`
* Border‑radius: `12px`
* Padding: `24px`
* Shadow Rules:
  - Interactive / Highlight Card: `box‑shadow: 4px 4px 0px #000000;`
  - Normal Static Card: `box‑shadow: 2px 2px 0px #000000;`
* **Hover Effect (Interactive Cards)**:
  * Transform: `translate(-2px, -2px)`
  * Box‑shadow: `6px 6px 0px #000000`
  * Transition: `all 0.2s cubic‑bezier(0.4, 0, 0.2, 1)`

### 6.4 New SaaS Standard Components (Added)
#### Badge / Pill Tag
- radius‑full, padding: 4px 10px, font‑size: 12px
- Primary badge: bg `#f4dc26`, text black
- Danger badge: bg `#e92929`, text white

#### Alert Notification Banner
- Error alert: border‑left 4px solid `#e92929`
- Success alert: border‑left 4px solid `#229948`

#### Table
- Header background: `#FFFDF4`
- Row bottom border: `1px solid #5c3613`
- Hover row bg: `rgba(244, 220, 38, 0.08)`

#### Tabs Navigation
- Active tab bottom border thick 3px `#f4dc26`

---

## 7. CSS Variables & SCSS / Sass Token Reference

### Root CSS Custom‑Properties
:root {
  /* Pikachu Core Brand Palette */
  --pk‑yellow‑primary: #f4dc26;
  --pk‑yellow‑flash: #ffe62d;
  --pk‑red‑thunder: #e92929;
  --pk‑brown‑fur: #5c3613;
  --pk‑black‑outline: #000000;

  /* Surfaces & Backgrounds */
  --bg‑canvas: #FFFDF4;
  --bg‑surface: #FFFFFF;

  /* Extended Neutral Scale */
  --neutral‑100: #f7f5ec;
  --neutral‑200: #e5e5e5;
  --neutral‑300: #cccccc;
  --neutral‑400: #888888;

  /* Functional Status Colors */
  --color‑success: #229948;
  --color‑info: #2469d8;
  --color‑warning: #f29c1f;

  /* Typography */
  --font‑main: 'Inter', system‑ui, -apple‑system, sans‑serif;

  /* Neo‑Brutalist Offset Shadow System (Graded) */
  --shadow‑neo‑sm: 2px 2px 0px #000000;
  --shadow‑neo: 4px 4px 0px #000000;
  --shadow‑neo‑lg: 6px 6px 0px #000000;
}

### SCSS / SASS Variables File (_pikachu‑tokens.scss)
// ========= Pikachu SaaS Design Tokens =========
// Brand Colors
$pk‑yellow‑primary: #f4dc26;
$pk‑yellow‑flash: #ffe62d;
$pk‑red‑thunder: #e92929;
$pk‑brown‑fur: #5c3613;
$pk‑black‑outline: #000000;

// Surface Background
$bg‑canvas: #FFFDF4;
$bg‑surface: #FFFFFF;

// Neutrals
$neutral‑100: #f7f5ec;
$neutral‑200: #e5e5e5;
$neutral‑300: #cccccc;
$neutral‑400: #888888;

// Functional Status
$color‑success: #229948;
$color‑info: #2469d8;
$color‑warning: #f29c1f;

// Spacing 8‑pt grid
$space‑2xs: 4px;
$space‑xs: 8px;
$space‑sm: 12px;
$space‑md: 16px;
$space‑lg: 24px;
$space‑xl: 32px;
$space‑2xl: 48px;
$space‑3xl: 64px;

// Border radius
$radius‑none: 0px;
$radius‑sm: 4px;
$radius‑md: 8px;
$radius‑lg: 16px;
$radius‑xl: 12px;
$radius‑full: 9999px;

// Shadows
$shadow‑neo‑sm: 2px 2px 0px #000000;
$shadow‑neo: 4px 4px 0px #000000;
$shadow‑neo‑lg: 6px 6px 0px #000000;

// Font stack
$font‑main: 'Inter', system‑ui, -apple‑system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans‑serif;

---

## 8. Implementation Best‑Practice & Forbidden‑List (Optimized New Section)
### ✅ Recommended
1. Yellow = action & highlight accent
2. Black hard border as signature UI feature
3. Graded shadow instead of universal heavy shadow
4. Warm off‑white canvas background to reduce eye fatigue

### ❌ Strictly Forbidden
1. Do NOT use `#e92929` red for non‑danger decorative UI elements
2. Do NOT fill large page blocks / full cards with primary yellow background
3. Do NOT use yellow for long‑form paragraph text
4. Do NOT add heavy `--shadow‑neo‑lg` shadow on every element (over‑crowded UI)

---

## 9. Style Final Positioning Summary
> Energetic Neo‑Brutalist SaaS UI
> Brand identity comes from yellow accent + thick black outline + offset hard shadow, NOT cartoon illustrations.
