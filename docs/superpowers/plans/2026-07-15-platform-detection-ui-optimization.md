# Platform Detection UI Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Tauri workbench fit its default window, split Douyin and Toutiao into truthful platform-specific detection cards, group batch actions, improve table readability, and make both dialogs keyboard-correct.

**Architecture:** Keep the existing Rust commands and result model. Move platform option construction and small display rules into pure TypeScript helpers, keep React orchestration in `App.tsx`, and add one reusable modal hook. CSS makes the center panel fluid and contains horizontal scrolling inside the table.

**Tech Stack:** React 19, TypeScript 5.8, Vite 7, Tauri 2, Node built-in test runner, CSS

## Global Constraints

- Douyin supports Token, password, certification, aid, and registration-time options.
- Toutiao supports only the existing login/certification check.
- No new Toutiao backend command is added.
- Existing CSV fields and Rust result models remain unchanged.
- Only one platform batch runs at a time.
- The page must not scroll horizontally at the default 1440px Tauri window size.
- The current checkout has no Git metadata, so commit steps are intentionally omitted.

---

## File Structure

- Modify `src/batch-options.ts`: platform option types, queued event helper, run-option conversion.
- Create `src/workbench-ui.ts`: duration and row-status presentation helpers.
- Create `src/use-modal-dialog.ts`: focus entry, focus trapping, Escape handling, scroll locking, focus restoration.
- Modify `src/App.tsx`: grouped actions, platform cards, table columns, status badges, empty state, dialog semantics.
- Modify `src/App.css`: fluid layout, action groups, platform cards, sticky identity columns, badges, dialog focus styling.
- Modify `tests/batch-options.test.ts`: platform conversion and queued-event regressions.
- Create `tests/workbench-ui.test.ts`: duration and status tests.
- Create `tests/modal-dialog.test.ts`: focus-wrap tests.

---

### Task 1: Platform-Specific Detection Option Model

**Files:**
- Modify: `src/batch-options.ts`
- Modify: `tests/batch-options.test.ts`

**Interfaces:**
- Produces: `DetectionPlatform`, `DouyinDetectionOptions`, `ToutiaoDetectionOptions`, `BatchDetectionOptions`, `buildBatchDetectionOptions(platform, douyin, toutiao)`.
- Preserves: `queueBatchOptionFromEvent(dispatch, key, event, readValue)` with a generic state type.

- [ ] **Step 1: Add failing tests for platform conversion**

```ts
const douyin = {
  token: true,
  password: false,
  certification: true,
  aid: true,
  registrationTime: false,
};
const toutiao = { certification: true };

test("builds a Douyin run from Douyin controls", () => {
  assert.deepEqual(buildBatchDetectionOptions("douyin", douyin, toutiao), {
    appType: "douyin", ...douyin,
  });
});

test("builds a Toutiao run without Douyin-only checks", () => {
  assert.deepEqual(buildBatchDetectionOptions("toutiao", douyin, toutiao), {
    appType: "toutiao",
    token: false,
    password: false,
    certification: true,
    aid: false,
    registrationTime: false,
  });
});
```

- [ ] **Step 2: Verify RED**

Run: `npm test`

Expected: FAIL because the conversion function and platform types do not exist.

- [ ] **Step 3: Implement the minimal option model**

```ts
export type DetectionPlatform = "douyin" | "toutiao";
export type DouyinDetectionOptions = {
  token: boolean;
  password: boolean;
  certification: boolean;
  aid: boolean;
  registrationTime: boolean;
};
export type ToutiaoDetectionOptions = { certification: boolean };
export type BatchDetectionOptions = DouyinDetectionOptions & {
  appType: DetectionPlatform;
};

export function buildBatchDetectionOptions(
  platform: DetectionPlatform,
  douyin: DouyinDetectionOptions,
  toutiao: ToutiaoDetectionOptions,
): BatchDetectionOptions {
  if (platform === "douyin") return { appType: platform, ...douyin };
  return {
    appType: platform,
    token: false,
    password: false,
    certification: toutiao.certification,
    aid: false,
    registrationTime: false,
  };
}
```

Generalize `queueBatchOptionFromEvent` over `State` and preserve synchronous primitive capture.

- [ ] **Step 4: Verify GREEN**

Run: `npm test`

Expected: queued-event and new platform tests all PASS.

---

### Task 2: Separate Douyin and Toutiao Detection Cards

**Files:**
- Modify: `src/App.tsx:374-700`
- Modify: `src/App.tsx:702-868`
- Modify: `src/App.tsx:1212-1265`
- Modify: `src/App.css:592-643`
- Modify: `src/App.css:1219-1350`

**Interfaces:**
- Consumes Task 1 platform option types and conversion helper.
- Produces: `handleRunBatchDetection(platform: DetectionPlatform)` and two visible platform cards.

- [ ] **Step 1: Replace shared state with independent platform states**

```ts
const [douyinOptions, setDouyinOptions] = useState<DouyinDetectionOptions>({
  token: true,
  password: true,
  certification: true,
  aid: true,
  registrationTime: true,
});
const [toutiaoOptions, setToutiaoOptions] = useState<ToutiaoDetectionOptions>({
  certification: true,
});
```

Compute Douyin and Toutiao counts from `trackedApps` and `getTrackedAppType`.

- [ ] **Step 2: Make batch start platform-explicit**

```ts
async function handleRunBatchDetection(platform: DetectionPlatform) {
  const runOptions = buildBatchDetectionOptions(
    platform, douyinOptions, toutiaoOptions,
  );
  const initialRows = buildInitialBatchRows(trackedApps, platform);
  // Keep the existing worker loop and pass runOptions to each row.
}
```

The worker closure must use the immutable `runOptions`, not live React state.

- [ ] **Step 3: Render the two cards**

The Douyin card contains five checkboxes and `开始检测抖音`. The Toutiao card contains only `登录/实名状态` and `开始检测头条`. Both show matching scanned APP counts. A shared `停止当前检测` button follows both cards.

- [ ] **Step 4: Apply running and empty-state rules**

Disable both card settings and starts while a batch runs. Disable a platform start when its scanned count is zero. Keep the shared stop enabled only during a run.

- [ ] **Step 5: Style cards responsively**

Use a blue-violet Douyin accent and orange-red Toutiao accent. Stack cards in the default right rail; at the 1320px bottom-rail layout show two columns; below 720px stack again.

- [ ] **Step 6: Verify**

Run: `npm test && npm run build`

Expected: tests PASS and build exits 0.

---

### Task 3: Fluid Layout, Grouped Actions, and Readable Table

**Files:**
- Create: `src/workbench-ui.ts`
- Create: `tests/workbench-ui.test.ts`
- Modify: `src/App.tsx:298-368`
- Modify: `src/App.tsx:450-650`
- Modify: `src/App.css:439-590`
- Modify: `src/App.css:1198-1375`

**Interfaces:**
- Produces: `formatDetectionDuration(durationMs)` and `getDetectionStatusLabel(status)`.

- [ ] **Step 1: Write failing helper tests**

```ts
test("formats short durations in milliseconds", () => {
  assert.equal(formatDetectionDuration(842), "842 ms");
});
test("formats long durations in seconds", () => {
  assert.equal(formatDetectionDuration(1420), "1.42 s");
});
test("formats detection statuses in Chinese", () => {
  assert.equal(getDetectionStatusLabel("checking"), "检测中");
  assert.equal(getDetectionStatusLabel("online"), "在线");
});
```

- [ ] **Step 2: Verify RED**

Run: `npm test`

Expected: FAIL because `src/workbench-ui.ts` does not exist.

- [ ] **Step 3: Implement helpers**

`formatDetectionDuration(null)` returns `-`; values below 1000 return rounded milliseconds; values at or above 1000 return seconds with at most two decimals. Status mappings are `待检测`, `检测中`, `在线`, `掉线`, `失败`, `已跳过`.

- [ ] **Step 4: Reorder and enhance the table**

Render first columns as `序号`, `状态`, `APP`, `账号`, then the remaining existing columns. Add a status badge and use the duration helper. Update empty-row `colSpan` from 32 to 33. Keep exported CSV order unchanged.

- [ ] **Step 5: Group the left rail**

Create labelled sections `列表`, `数据导出`, and `包分配`. Rename `清空列表框` to `清空列表`, disable it when rows are empty, and use danger styling when enabled.

- [ ] **Step 6: Add an actionable empty state**

Render a heading, short instruction, and a unique `扫描资源` button calling `onOpenScanModal` inside the empty table state.

- [ ] **Step 7: Contain horizontal scrolling**

```css
.app-shell-v3 {
  width: 100%;
  max-width: none;
  padding: 18px;
  overflow: hidden;
}
.app-shell-v3 .detector-workbench {
  width: 100%;
  grid-template-columns: minmax(118px, 136px) minmax(0, 1fr) minmax(220px, 244px);
}
.app-shell-v3 .detector-table-panel {
  width: auto;
  max-width: none;
  min-width: 0;
}
```

Keep `overflow: auto` on `.detector-table-scroll` and make the first four columns sticky with explicit widths and offsets.

- [ ] **Step 8: Verify GREEN and build**

Run: `npm test && npm run build`

Expected: all tests PASS and build exits 0.

---

### Task 4: Accessible Scan and Detail Dialogs

**Files:**
- Create: `src/use-modal-dialog.ts`
- Create: `tests/modal-dialog.test.ts`
- Modify: `src/App.tsx:702-744`
- Modify: `src/App.tsx:1344-1361`
- Modify: `src/App.tsx:1773-1886`
- Modify: `src/App.css:1377-1557`

**Interfaces:**
- Produces: `getWrappedFocusIndex(currentIndex, count, backwards)` and `useModalDialog({open, canClose, onRequestClose})`.
- Hook returns: `{ dialogRef, onDialogKeyDown }`.

- [ ] **Step 1: Write failing focus-wrap tests**

```ts
test("wraps forward focus", () => {
  assert.equal(getWrappedFocusIndex(3, 4, false), 0);
});
test("wraps backward focus", () => {
  assert.equal(getWrappedFocusIndex(0, 4, true), 3);
});
test("moves focus within the dialog", () => {
  assert.equal(getWrappedFocusIndex(1, 4, false), 2);
});
```

- [ ] **Step 2: Verify RED**

Run: `npm test`

Expected: FAIL because `src/use-modal-dialog.ts` does not exist.

- [ ] **Step 3: Implement focus management**

The hook captures the opener, locks body scrolling, focuses `[data-modal-autofocus]` or the dialog, closes on Escape when allowed, traps Tab/Shift+Tab among enabled visible controls, restores body overflow, and restores focus on cleanup.

- [ ] **Step 4: Apply dialog semantics**

Both modal sections receive `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, `tabIndex={-1}`, the hook ref, and the hook key handler. Each heading receives its matching ID. Each close button receives `data-modal-autofocus`.

- [ ] **Step 5: Improve scan copy**

Use `选择 ZIP 文件或目录，扫描后即可按平台开始检测。` and rename `扫描路径` to `开始扫描`.

- [ ] **Step 6: Verify**

Run: `npm test && npm run build`

Expected: focus tests PASS and build exits 0.

---

### Task 5: Packaged-App Verification

**Files:**
- Verify: `src/App.tsx`
- Verify: `src/App.css`
- Verify: `src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app`

- [ ] **Step 1: Run the full source stack**

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: zero Node failures, Vite build exit 0, and all Rust tests pass.

- [ ] **Step 2: Build the macOS app**

Run: `npm run tauri build -- --bundles app`

Expected: the `.app` bundle is refreshed without using the unrelated DMG path.

- [ ] **Step 3: Inspect the default workbench**

Verify no page-level horizontal scrollbar; grouped left actions; both platform cards; truthful options; platform-labelled starts; table scrolling contained in the center panel.

- [ ] **Step 4: Exercise controls**

Toggle at least two Douyin options and the Toutiao login/certification option. Confirm the values update and the UI remains rendered.

- [ ] **Step 5: Exercise modal keyboard behavior**

Open the scan dialog, cycle Tab and Shift+Tab, press Escape, and confirm focus returns to `扫描资源`. Repeat with a detail dialog when a populated row is available.

- [ ] **Step 6: Capture evidence**

Capture default-workbench and scan-dialog screenshots. Record the populated-detail limitation if no ZIP fixture is available.

---

## Self-Review Checklist

- [x] Spec coverage: platform cards, grouped actions, responsive shell, table readability, modal keyboard behavior, copy, and packaged QA are covered.
- [x] Placeholder scan: no unresolved implementation placeholder remains.
- [x] Type consistency: platform types, flat run options, helper names, and hook return names match across tasks.
- [x] Scope: no Toutiao backend expansion, CSV change, or virtualization rewrite is included.
