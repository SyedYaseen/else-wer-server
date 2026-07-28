# 8. Frontend Test Coverage

Source: business/GA audit, /home/loop/.claude/plans/do-a-business-audit-cheeky-orbit.md (2026-07-11).
Context: `src/ui` has zero tests (no Vitest/Jest config, no `.test.`/`.spec.` files, no test script in
package.json). Rust side already has inline `#[test]` coverage in db/file_ops/services modules; this
gap is frontend-only. Matters once contributors other than the author start sending PRs.

## Todo
- [ ] Add a test runner (Vitest is the natural fit for a Vite project) + config + `test` script in
  `src/ui/package.json`.
- [ ] Cover `offline/downloadStore.ts` and `offline/storage.ts` (IndexedDB v3 segmented download
  manager) - highest risk, hardest to manually verify, silent-corruption-prone.
- [ ] Cover `player/engine.ts` (singleton Audio()-based playback engine) - resume/seek/speed logic.
- [ ] Cover Organize mutation flows (`OrganizePage.tsx` + `save_organized_files` client calls) -
  rename/move/merge are destructive-ish actions gated by ConfirmActionSheet; regressions here are
  high-blast-radius (can rewrite user's file layout).

## Done
(none yet)
