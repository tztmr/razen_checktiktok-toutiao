# V3 Workbench Redesign Design

Date: 2026-06-23
Project: `ios_zen_plist_read`
Surface: Tauri desktop home screen
Status: Approved in conversation, awaiting user review of written spec

## Goal

Replace the current V2 home screen with a V3 "workbench-only" desktop UI.

The new home screen should:

- remove the current hero banner
- remove the always-visible path toolbar
- remove the summary-card strip as a separate visual section
- remove the always-visible `APP 列表 / 候选文件 / 解析结果` three-column workspace
- keep the batch detection table as the primary permanent surface
- move scan entry into a centered modal
- open row details in a large centered modal
- show row details with tab navigation

The intended result is a focused operator console: users land directly in the detection workbench instead of first seeing onboarding-style or dashboard-style scaffolding.

## Final Decisions

The approved V3 direction is:

1. Home screen keeps only the core workbench.
2. Workbench layout remains three-part:
   - left action rail
   - center table area
   - right detection-options rail
3. Scan entry opens in a centered modal.
4. Clicking a detection row opens a large detail modal.
5. Detail modal uses top tabs:
   - `账号总览`
   - `候选文件`
   - `解析结果`
   - `原始数据`

## Problems In V2

The current V2 home screen mixes two different jobs:

- "prepare a scan"
- "operate the detection table"

That causes four UX issues:

1. The hero section makes the screen feel like a landing page instead of a working tool.
2. The visible path toolbar takes first-screen space even after scan setup is complete.
3. The separate summary cards and lower three-column workspace compete with the batch table.
4. Scan and detail flows live permanently on screen even when the user only wants to inspect or detect accounts.

V3 fixes this by making the batch table the center of gravity and turning setup/detail into temporary overlays.

## Information Architecture

### Persistent Home Screen

The home screen contains only one major surface:

- `WorkbenchShell`

Inside `WorkbenchShell`:

- `ActionRail` on the left
- `DetectionTablePanel` in the center
- `DetectionConfigRail` on the right

### Temporary Overlays

The home screen can open two modal layers:

- `ScanSetupModal`
- `DetectionDetailModal`

The three-column V2 lower workspace is removed from the base page. Its information does not disappear; it is redistributed into the detail modal.

## V3 Layout

### Left Action Rail

Purpose:

- keep high-frequency utility actions visible
- preserve the operator-console feel from the current detection panel

Expected actions:

- `清空列表框`
- `导出全部数据`
- `导出掉线数据`
- `导出在线数据`
- `分配在线包`
- `分配掉线包`
- `分配正常功能包`
- `分配限制功能包`

Design notes:

- vertical stacked buttons
- compact width
- no extra summary cards or headings beyond a minimal rail label if needed
- secondary visual weight compared to the center table

### Center Detection Table Panel

Purpose:

- become the permanent main surface
- own the page's status summary and table interactions

Subsections:

1. Top utility bar
2. Sticky table header
3. Scrollable detection table body

Top utility bar content:

- compact status summary:
  - 累计检测
  - 在线数量
  - 掉线数量
  - 累计用时
- scan entry trigger button
- optional secondary utility button if needed for quick refresh or re-scan

Important rule:

The old standalone summary-card strip is removed. Summary data moves into the center table toolbar so status remains visible without consuming another full section.

### Right Detection Config Rail

Purpose:

- keep detection options always available while preserving the current operator workflow

Contents:

- app target selector
- token checkbox
- password checkbox
- certification checkbox
- aid checkbox
- registration time checkbox
- `开始检测`
- `停止检测`

Design notes:

- fixed narrow rail
- visually lighter than the center table
- CTA button remains visually prominent

## Modal Flows

### ScanSetupModal

Trigger:

- opened from the center toolbar action

Primary responsibilities:

- enter or paste a ZIP or directory path
- pick directory
- pick ZIP
- optionally support drag/drop within the modal
- start scanning

Behavior:

- modal opens centered over the workbench
- modal owns scan-related controls that were previously always visible
- on successful scan, modal closes and the workbench updates
- on scan error, modal stays open and shows the error/status message

Contents:

- path input
- helper text
- `选目录`
- `选 ZIP`
- `扫描路径`
- optional status area for scan progress / path validation

Resolved behavior in this spec:

- scan progress remains available in the modal first
- if a long-running scan is active, center toolbar summary can also reflect scanning state

### DetectionDetailModal

Trigger:

- click a row in the batch detection table

Purpose:

- replace the old permanent lower workspace
- keep details available without bloating the home screen

Structure:

1. modal header
2. top tab row
3. summary meta strip
4. tab content area

Header content:

- account/app identity
- source ZIP
- current detection status
- close action

Top tabs:

- `账号总览`
- `候选文件`
- `解析结果`
- `原始数据`

#### Tab: `账号总览`

Shows a concise operator overview from the clicked detection row:

- APP
- account name
- phone number
- register time
- aid
- token status
- password / real-name status
- child-lock status
- online/offline result
- normal functions
- limited functions
- duration
- source ZIP basename

This tab is the fast read. Users should understand the account status without digging into candidate files first.

#### Tab: `候选文件`

Reuses the current candidate-file capability but moves it into modal scope.

Contents:

- file list for the selected row's app
- file type
- scope
- size
- parse support

Interaction:

- selecting a file updates modal-local selection state
- selecting a file also prepares the `解析结果` and `原始数据` tabs

#### Tab: `解析结果`

Reuses current parse rendering logic.

It must continue to support:

- cookie specialized views
- Douyin preferences specialized views
- Toutiao preferences specialized views
- sqlite table browsing
- generic preference cards
- generic JSON fallback

Important constraint:

The parsing UI should move into modal tab content, not be rewritten from scratch unless necessary. Existing logic should be extracted and hosted inside a new modal content container.

#### Tab: `原始数据`

Purpose:

- provide operator-level debugging and trustability
- expose raw payloads without mixing them into the polished parse view

Possible content:

- raw `parseResult.parsedData`
- raw endpoint payload snippets
- raw selected file metadata
- raw cookies or assembled request headers when relevant

This tab intentionally absorbs the "I need to inspect the actual source" use case that otherwise expands the main UI.

## Component Architecture

The implementation should refactor the current large surface into focused UI units without changing the core data model more than necessary.

Target structure:

- `App`
- `WorkbenchShell`
- `ActionRail`
- `DetectionTablePanel`
- `DetectionConfigRail`
- `ScanSetupModal`
- `DetectionDetailModal`
- `DetailTabs`
- `DetailOverviewTab`
- `DetailFilesTab`
- `DetailParseTab`
- `DetailRawTab`

Likely extraction candidates from current code:

- `DetectorWorkbench` becomes the seed for:
  - `WorkbenchShell`
  - `ActionRail`
  - `DetectionTablePanel`
  - `DetectionConfigRail`
- current result rendering branches become reusable tab content sections

## State Model

### Keep

The redesign should preserve the current functional state where possible:

- `sourcePath`
- `scanSummary`
- `files`
- `selectedFile`
- `parseResult`
- `status`
- `loading`
- `scanProgress`
- `batchRows`
- `batchOptions`
- `batchRunning`
- `batchStartedAt`
- `batchElapsedMs`

### Add

New UI state is required:

- `isScanModalOpen`
- `isDetailModalOpen`
- `activeDetailTab`
- `selectedBatchRowKey`
- modal-local selected candidate file if separate from page-level selection

### Derived Relationships

The clicked batch row becomes the anchor record for the detail modal.

From that row, the UI derives:

- source ZIP
- app id
- app type
- account summary
- candidate file query target
- parse target context

The page should not require the old lower workspace selection model to remain visible.

## Data Flow

### Scan Flow

1. User opens `ScanSetupModal`.
2. User enters or selects path.
3. User starts scan.
4. Existing scan logic runs.
5. `scanSummary`, tracked apps, and file statistics update.
6. Modal closes on success.
7. Center toolbar summary reflects fresh totals.

### Detection Flow

1. User adjusts right-rail options.
2. User clicks `开始检测`.
3. Existing batch detection flow runs.
4. Center table updates row-by-row.
5. Left-rail export/move actions continue to use `batchRows`.

### Detail Flow

1. User clicks one table row.
2. `DetectionDetailModal` opens.
3. Overview tab renders immediately from row data.
4. Candidate files load for the row's `sourceZip + appId` if not already ready.
5. User chooses a file in `候选文件`.
6. Parse logic hydrates `解析结果`.
7. `原始数据` exposes raw payloads for the same selected file/context.

## Styling Direction

The V3 style should feel like a desktop tool, not a marketing page.

Visual principles:

- flatter hierarchy above the fold
- fewer decorative gradients
- no large welcome banner
- tighter but still polished spacing
- high information density in the center area
- modal layers that feel deliberate and professional

What to remove from V2:

- hero section styling
- large onboarding headline
- standalone summary tiles as a top strip
- permanent lower content cards for file/detail browsing

What to keep or evolve:

- soft light desktop palette
- clean rounded surfaces
- readable borders and shadows
- sticky, spreadsheet-like table behavior

## Error Handling

### Scan Errors

- shown inside `ScanSetupModal`
- modal remains open
- no silent close on failure

### Detection Errors

- row-level failures stay in the table
- detail modal can still open failed rows when useful
- top summary continues counting failed/offline states as today

### Parse Errors

- shown in `解析结果` tab
- `原始数据` tab still available when raw data exists

### Empty States

- center table empty state when no scan or no rows
- `候选文件` tab empty state when no tracked files exist
- `解析结果` empty state until a file is chosen
- `原始数据` empty state when no raw payload is available

## Non-Goals

This redesign does not change:

- Rust parsing logic
- batch detection business rules
- endpoint validation strategy
- Douyin/Toutiao extraction semantics
- export file formats
- package move semantics

This is a UI restructuring and experience simplification effort, not a backend feature rewrite.

## Migration Strategy

Recommended implementation order:

1. Remove V2 hero, toolbar section, summary-card strip, and bottom workspace from the page layout.
2. Split `DetectorWorkbench` into three persistent rails/panels.
3. Add centered `ScanSetupModal` and move scan controls into it.
4. Add row click handling and `DetectionDetailModal`.
5. Move existing detail-rendering logic into tab content.
6. Polish visual density and spacing.

This order minimizes risk because the table remains functional throughout.

## Testing And Verification Plan

Implementation should be verified with:

1. Build passes with `npm run build`.
2. Tauri dev run still launches successfully.
3. Scan modal can:
   - open
   - accept manual path input
   - select ZIP/directory
   - run scan
   - surface scan errors
4. Workbench still supports:
   - batch start
   - batch stop
   - export all/online/offline
   - move online/offline/normal/limited packages
5. Clicking a table row opens detail modal.
6. Detail modal tabs switch correctly.
7. Candidate file selection still drives parse rendering.
8. Specialized parse views still render for:
   - cookies
   - Douyin preferences
   - Toutiao preferences
   - sqlite
9. Empty states remain readable at first launch.
10. Large modal layouts remain usable at desktop window sizes typical for the Tauri app.

## Risks

1. Current `App.tsx` is large, so modal extraction may expose hidden coupling.
2. Existing detail rendering relies on page-level selection state and may need careful adaptation for modal-local flows.
3. Reusing parse state across tabs must avoid stale or cross-row leakage.
4. Removing the visible scan toolbar may require an extra affordance so first-time users can still find scan entry quickly.

## Recommendation

Proceed with implementation exactly against this V3 workbench structure.

The strongest principle for implementation is:

Keep the home screen focused on live detection work, and push setup/detail complexity into modals instead of keeping it permanently mounted on the page.
