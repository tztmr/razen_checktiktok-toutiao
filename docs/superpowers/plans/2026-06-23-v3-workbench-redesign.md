# V3 Workbench Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current V2 home screen with a workbench-only V3 UI that keeps the batch table on the main page, moves scan setup into a centered modal, and opens row details in a large tabbed modal.

**Architecture:** Keep existing scan, parse, batch-detection, and export logic in place, but reorganize the React surface around a new persistent workbench shell. Extract modal-oriented display helpers from the old always-visible workspace and reuse existing selected-file / parse-result logic wherever possible.

**Tech Stack:** React 19, TypeScript, Vite, Tauri 2, existing CSS in `src/App.css`

---

### Task 1: Create the execution skeleton

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Verify: `npm run build`

- [ ] Add UI state for the new scan modal, detail modal, active detail tab, and selected batch row.
- [ ] Add derived row-to-app helpers so a clicked batch row can drive file loading and parse state.
- [ ] Keep existing batch detection and parsing functions working before any visual polish.
- [ ] Run `npm run build` to make sure the new state additions compile before the larger layout rewrite.

### Task 2: Replace the V2 home shell with the V3 workbench

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Verify: `npm run build`

- [ ] Remove the hero banner, old scan toolbar, summary-card strip, and bottom three-column workspace from the main render tree.
- [ ] Reshape the page into `left action rail + center table panel + right config rail`.
- [ ] Move status summary into the table toolbar and add a scan-entry trigger there.
- [ ] Preserve current left-rail actions and right-rail detection options.
- [ ] Run `npm run build` after the shell rewrite.

### Task 3: Move scan setup into a centered modal

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Verify: `npm run build`

- [ ] Create a centered scan modal with path input, helper text, `选目录`, `选 ZIP`, and `扫描路径`.
- [ ] Reuse the existing scan flow so the modal opens, runs scan, surfaces progress/errors, and closes on success.
- [ ] Keep drag-and-drop support by routing dropped ZIP paths into the same scan state.
- [ ] Ensure the main page still shows enough status while a scan is running.
- [ ] Run `npm run build` after modal wiring.

### Task 4: Replace the lower workspace with a row-driven detail modal

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Verify: `npm run build`

- [ ] Add row click behavior to the detection table.
- [ ] Open a large centered detail modal for the clicked row.
- [ ] Render a header summary and top tabs: `账号总览`, `候选文件`, `解析结果`, `原始数据`.
- [ ] Move the old candidate-file list into the `候选文件` tab.
- [ ] Move the old parse-result rendering into the `解析结果` tab.
- [ ] Add a raw-data tab using the existing parse and metadata state.
- [ ] Run `npm run build` after modal detail content is connected.

### Task 5: Polish the visual system and validate behavior

**Files:**
- Modify: `src/App.css`
- Modify: `src/App.tsx`
- Verify: `npm run build`
- Verify: `npm run tauri dev`

- [ ] Align the UI with the approved V3 concept: flatter desktop-tool hierarchy, no marketing-style header, strong centered table focus, and deliberate modal styling.
- [ ] Check empty states for first launch, no-scan, no-files, and no-parse scenarios.
- [ ] Verify scan modal open/close flow, table row click flow, tab switching, and batch action availability.
- [ ] Run `npm run build`.
- [ ] Run `npm run tauri dev` and inspect the live app.

### Self-review

- [ ] Spec coverage check: main workbench shell, scan modal, detail modal, tab set, and removal of V2 persistent panels are all covered.
- [ ] Placeholder scan: no `TODO`, `TBD`, or unresolved placeholders remain in this plan.
- [ ] Type consistency check: modal state, selected row, selected file, and parse-result flow all use the existing `App.tsx` data model.
