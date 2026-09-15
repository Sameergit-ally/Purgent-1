# Frontend Design Prompts (shadcn + Tailwind + TypeScript)

Ye file 6 ready-to-use prompts hai jo tum apne AI coding agent (Claude/Cursor/etc.) ko de sakte ho ek-ek karke. Har prompt ek specific section banata hai, aur sabme same design system (color + type + spacing + radius) use hoga taki poori site ek cohesive product jaisi lage, alag-alag AI-generated pieces jaisi nahi.

---

## 🎨 Design System (paste this into every prompt / project's tailwind config)

Generic AI-look (cream + terracotta, ya plain black + neon-green, ya SaaS card kit with identical rounded shadows everywhere) jaanbujh kar avoid kiya gaya hai. Theme naam: **"Editorial Ink"** — warm-dark, premium, thoda print/editorial feel.

### Colors

| Token | Hex | Use |
|---|---|---|
| `background` | `#12130F` | Base page background |
| `surface` | `#1B1C16` | Cards, panels, raised surfaces |
| `surface-2` | `#22231C` | Nested/hover surface state |
| `foreground` | `#EDEAE0` | Primary text |
| `muted` | `#8C8A7C` | Secondary text, placeholders |
| `accent` | `#B8862F` | Primary CTA, links, active highlights (muted gold — not neon) |
| `accent-hover` | `#CC9A3F` | Hover state for accent elements |
| `accent-2` | `#5C7A5E` | Secondary accent — sage green, tags/success/one highlighted card only |
| `danger` | `#B0503C` | Errors, destructive actions (warm brick red, stays in-family) |
| `border` | `#2C2D26` | Hairline borders, dividers |

Use accent sparingly — one primary action per screen, not every icon/link tinted gold. That restraint is what keeps it looking designed, not decorated.

### Typography

- **Display/Headline:** a serif with real character — "Fraunces" or "Source Serif 4". Use at 500–600 weight for headlines, never default 400.
- **Body/UI:** a clean grotesque sans — "Inter" or "General Sans", 400/500 weight.
- **Mono (optional, for data/labels only, not decoration):** "IBM Plex Mono" or "JetBrains Mono".
- **Scale** (fluid, roughly 1.25 ratio): `text-sm` 14px → `text-base` 16px → `text-xl` 20px → `text-3xl` 30px → `text-5xl` 48px → `text-6xl/7xl` 60–72px for hero headlines.
- Line length under ~75 characters for body copy. Serif body text gets slightly more line-height (1.6+) than sans.

### Spacing & Shape

- Spacing scale: multiples of 4px (Tailwind default) — stick to it, don't invent one-off px values.
- Radius: **not one blanket radius everywhere.** Small elements (buttons, inputs, tags) → `rounded-md` (6px). Cards/panels → `rounded-xl` (12px). Only the one "hero" visual element can go bigger (`rounded-2xl`+) if it's the intentional focal point.
- Shadows: avoid the generic `shadow-md` grey box-shadow on every card. Prefer a 1px `border` (#2C2D26) for separation; reserve shadow for genuinely elevated/floating elements (modals, dropdowns) only.

### Motion

One deliberate motion moment per section max (e.g. hero entrance, or a hover reveal on the one dominant bento card) — not fade-slide-up on every element and not a hover transition on every single card. Always respect `prefers-reduced-motion`.

### What to avoid (generic AI tells — checklist for every prompt)
- ALL-CAPS eyebrow labels above every heading
- "→" appended to every link/button
- Bolding/italicizing a single word in a headline for "emphasis"
- Numbered 01/02/03 markers on content that isn't actually a sequence
- Identical rounded-card + identical soft-shadow on every content block regardless of hierarchy
- Middle-dot separated meta strings ("A · B · C")

---

## Prompt 1 — Hero Section

```
You are given a task to integrate/build a Hero section component in the codebase.

The codebase should support shadcn project structure, Tailwind CSS, and TypeScript.
If it doesn't, provide setup instructions first (shadcn CLI, Tailwind, TypeScript).

Determine the default path for components (/components/ui). If it doesn't exist, create it.

Build a Hero section with:
- Eyebrow label + headline (serif, large, tight tracking) + supporting paragraph
- Primary CTA button + secondary ghost/link CTA
- One deliberate visual moment (product screenshot, illustration, or subtle motion) — not a generic gradient blob
- Fully responsive (mobile-first), visible keyboard focus states, respects prefers-reduced-motion
- Exactly one deliberate entrance motion moment — nothing fading/sliding on every child element

Use this design system (Editorial Ink theme):
- Colors: background #12130F, surface #1B1C16, foreground #EDEAE0, muted #8C8A7C, accent #B8862F, accent-hover #CC9A3F, accent-2 #5C7A5E, border #2C2D26
- Type: serif display (Fraunces/Source Serif 4, 500-600 weight) for headline, sans (Inter/General Sans) for body/UI
- Radius: rounded-md on buttons/inputs, rounded-xl on cards, larger radius only on the hero's own focal visual
- No generic grey box-shadow on cards — use border #2C2D26 for separation instead

Steps:
1. Set up Tailwind config with these tokens as CSS variables
2. Build the component in /components/ui/hero.tsx with a demo.tsx
3. Use lucide-react for any icons
4. Avoid generic AI tells: no ALL-CAPS eyebrow, no "→" on every button, no single-word bold/italic in headline, no numbered markers unless content is truly a sequence
```

---

## Prompt 2 — Navbar

```
You are given a task to build a responsive Navbar component in the codebase (shadcn + Tailwind + TypeScript).

Requirements:
- Logo/wordmark left, nav links center or right (your call, justify the choice), CTA button
- Mobile: hamburger menu with slide-down panel, accessible (aria-label, focus trap on open)
- Sticky on scroll with a subtle backdrop-blur + border-bottom that appears only on scroll (not from the start)
- Use the Editorial Ink design system: background #12130F, foreground #EDEAE0, muted #8C8A7C, accent #B8862F, border #2C2D26, sans font (Inter/General Sans) for nav labels, rounded-md on the CTA button

Steps:
1. Confirm /components/ui exists, else create it
2. Build /components/ui/navbar.tsx + demo.tsx
3. Use React state for mobile menu toggle, lucide-react for menu/close icons
4. Keep it framework-agnostic React (no Next.js-only APIs unless codebase is Next.js — check first)
```

---

## Prompt 3 — Features / Bento Grid

```
You are given a task to build a Features section (bento-style grid) in the codebase (shadcn + Tailwind + TypeScript).

Requirements:
- 4-6 feature cards in an asymmetric bento grid (not all identical size — one card should be visually dominant)
- Each card: icon (lucide-react), short title, 1-2 line description
- Avoid the "SaaS-card kit" default: don't put the same soft grey shadow + same border-radius on every card with no hierarchy
- Use accent-2 (#5C7A5E) sparingly for one highlighted card only, rest neutral with surface #1B1C16, rounded-xl, border #2C2D26 for separation (no drop shadows)
- Headline in serif (Fraunces/Source Serif 4), card titles in sans, body copy in sans muted (#8C8A7C)

Steps:
1. Build /components/ui/features-grid.tsx + demo.tsx
2. Write real, specific copy for each feature (no lorem ipsum, no "Powerful. Simple. Fast." filler)
3. Responsive: stack to single column on mobile
```

---

## Prompt 4 — Pricing Section

```
You are given a task to build a Pricing section in the codebase (shadcn + Tailwind + TypeScript).

Requirements:
- 3 tiers (Starter, Pro, Enterprise-style — rename to fit product), one marked "recommended" using accent #B8862F border/highlight
- Monthly/yearly toggle (React state) with a small "save X%" note on yearly
- Feature list per tier using check icons (lucide-react), muted #8C8A7C for unavailable features (not just greyed strikethrough)
- CTA button per tier, primary style (accent #B8862F fill, rounded-md) only on the recommended tier — others get a quiet outline/ghost style
- Cards: surface #1B1C16, border #2C2D26, rounded-xl; recommended card gets an accent-colored border instead of a shadow to stand out

Steps:
1. Build /components/ui/pricing.tsx + demo.tsx
2. Keep numbers/copy realistic and specific to the product context
3. Fully responsive: cards stack vertically on mobile, recommended tier stays visually first
```

---

## Prompt 5 — CTA + Footer

```
You are given a task to build a closing CTA band + Footer in the codebase (shadcn + Tailwind + TypeScript).

Requirements:
- CTA band: bold serif headline, one primary button, background using accent-2 #5C7A5E or a subtle accent-tinted surface (not a loud gradient)
- Footer: logo, 3-4 link columns, newsletter input (optional), copyright line, social icons (lucide-react)
- Border-top using border #2C2D26, background #12130F, muted #8C8A7C for secondary footer text
- Footer link labels in sans, logo/wordmark in serif to match hero/navbar

Steps:
1. Build /components/ui/cta-footer.tsx + demo.tsx
2. Keep footer links realistic/specific to product sections already built (Features, Pricing, Docs, etc.)
3. Responsive: footer columns stack on mobile
```

---

## Prompt 6 — Testimonials / Social Proof + FAQ

```
You are given a task to build a Testimonials + FAQ section in the codebase (shadcn + Tailwind + TypeScript).

Requirements:
- Testimonials: 3-4 real-sounding quotes with name, role, company — avoid generic "This changed everything!" copy, write specific outcome-based quotes
- Layout: not a generic 3-equal-card grid with identical shadows — consider a single large featured quote + smaller supporting ones, or a horizontal scroll strip
- FAQ: accordion using shadcn Accordion primitive if available, else build accessible accordion (aria-expanded, keyboard operable)
- 5-6 realistic questions specific to the product, not filler ("What is X?" / "How much does it cost?" generic pairs)

Design system: surface #1B1C16 for quote cards, border #2C2D26, accent #B8862F only on the featured quote's border or the open accordion item, serif for the featured quote text, sans everywhere else.

Steps:
1. Build /components/ui/testimonials-faq.tsx + demo.tsx
2. Write copy yourself, grounded and specific — no lorem ipsum
3. Responsive: single column on mobile, accordion works with touch
```

---

## Final QA Pass (run after all 6 are built)

```
Review the full page (Navbar, Hero, Features, Pricing, Testimonials/FAQ, CTA+Footer) built so far as one cohesive product, not 6 separate pieces.

Check for:
1. Design token drift — any hardcoded colors/fonts that don't match the Editorial Ink system? Fix them.
2. Radius/shadow consistency — same radius rules and no stray box-shadows across all sections.
3. Vertical rhythm — consistent section padding (pick one scale, e.g. py-20 md:py-32, and apply everywhere).
4. Accessibility — keyboard focus visible everywhere, sufficient color contrast (test foreground #EDEAE0 and muted #8C8A7C against background #12130F), alt text on images, reduced-motion respected.
5. Repetition — is the same layout pattern (e.g. icon + title + paragraph) repeated identically in 3+ sections? Vary at least one for visual rhythm.
6. Mobile pass — actually check every section stacks and reads well under 400px width.

Fix issues directly, then summarize what was changed.
```

---

### Tip
Sabhi 6 prompts same chat/agent session me ek-ek karke do (poora section complete hone ke baad hi next bhejo), taki components ek doosre se visually match karein aur design tokens repeat na likhne pade baar baar. Sabse aakhir me "Final QA Pass" prompt zaroor chalao — wahi cheez pura page ko "6 alag AI outputs" se "ek design" me badalta hai.