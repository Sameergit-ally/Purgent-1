# Purgent Frontend — To-Do List

Based on code review of `src/App.tsx` and `src/components/ui/*`. Grouped by priority.

---

## 🔴 P0 — Functional Bugs (fix first)

- [x] **Fix busy-indicator key mismatch.** `runWipe`/`runErase`/`runCarve` set `busy[frontend-generated-key] = true`, but the `operation-complete` / `operation-error` event listeners clear busy using the backend's `operation_id` — a different key. Success path never clears the flag. **Fix:** added a `clearBusy(id)` helper that *deletes* keys; handlers clear both the frontend key (on success/error) and the listeners clear by backend `operation_id`, so no stale keys linger.

- [x] **Fix broken path placeholders.** Real tab/form-feed characters were rendering from `\t` in JSX string attributes.
  **Fix:** all five placeholders (wipe → file target, erase file/folder, carve → source, carve → output) now use double-backslash literal strings (`{"C:\\path\\to\\..."}`).

- [x] **Remove duplicate IPC calls on load.** `get_wipe_standards` and `get_erase_standards` were each invoked twice in the init `useEffect`. **Fix:** single fetch per command; default derived from `list[0]?.id`.

- [x] **Filter hardware wipe standards by target type.** `ata_secure_erase`/`nvme_sanitize` are now filtered out of the Standard dropdown when `wipeTargetKind === "file"` (with a hint if a previously-selected hardware standard is still active).

- [x] **Replace `alert()` in `exportXml`** with a dismissible green success banner (`.banner.success` added to `styles.css`).

---

## 🟠 P1 — Content Accuracy (marketing copy vs. real backend)

- [x] **Correct the Ed25519 claims in the FAQ.** Copy now says the reports are **HMAC-SHA256** signed (with an explicit "Ed25519 is on the roadmap" note) — matching the backend exactly.

- [x] **Remove the "Destroy category" auto-note claim in the FAQ.** No such logic exists in the wipe pipeline. FAQ now describes the real behavior: skipped sectors are logged per-LBA with a running count, and the signed report records the final count — no automatic threshold verdict.

- [x] **Replace fabricated testimonials.** Section renamed to **"Illustrative Use-Case Scenarios"** with a visible placeholder disclaimer; fake names/orgs/metrics replaced with generic role-based attributions ("Illustrative use case — replace with verified results").

- [x] **Fix pricing tier CTA buttons.** All three CTAs now open `mailto:sales@purgent.dev` links with per-tier subjects instead of launching the console.

- [x] **Stop advertising license-tier enforcement.** Non-existent features (PKCS#11 HSM, custom signing keys, multi-operator roles, custom firmware profiles, compliance SLA) are marked **(Planned)**; the `included:false` gating implication was removed and a "Demo pricing model" footnote added. Supabase sync is described as env-gated opt-in (available regardless of tier).

## 🟡 P2 — Accessibility

- [x] **Associate all form labels with their inputs.** All 11 console `<label>` elements now have `htmlFor`/`id` pairing (operator identity, wipe target type/path/standard/confirm, erase target type/path/standard/confirm, carve source/output).

- [x] **Fix duplicate navbar anchor.** Removed the duplicate "Architecture" link; nav is now `Capabilities → #features` and `Compliance & FAQ → #faq` (also fixes the dangling `#standards` target).

- [x] **Prevent default on in-app anchor nav links.** `handleLinkClick` now calls `e.preventDefault()`, so only the custom smooth-scroll fires.

## 🟢 P3 — Performance / Polish

- [x] **Isolate the clock into its own component.** New `src/components/ui/Clock.tsx` owns the 1s `setInterval` and renders only the UTC cell; the top-level `App` no longer re-renders the whole console every second.

---

## Quick Reference — Files to Touch

| File | Items |
|---|---|
| `src/App.tsx` | P0 all items, P2 label associations, P3 clock removal |
| `src/components/ui/Clock.tsx` | **New** — P3 isolated clock |
| `src/components/ui/navbar.tsx` | P2 duplicate anchor + preventDefault |
| `src/components/ui/testimonials-faq.tsx` | P1 testimonials + FAQ copy fixes |
| `src/components/ui/pricing.tsx` | P1 CTA wiring / tier gating decision |
| `src/components/ui/landing-page.tsx` | P1 — mounts Pricing + TestimonialsFaq so nav anchors resolve |
