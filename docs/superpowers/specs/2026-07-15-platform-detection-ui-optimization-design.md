# Platform Detection UI Optimization Design

Date: 2026-07-15
Project: `ios_zen_plist_read`
Surface: Tauri desktop detection workbench
Status: Approved in conversation, pending written-spec review

## Goal

Improve the V3 workbench so the default desktop window fits without page-level horizontal scrolling, batch actions are visibly grouped, Douyin and Toutiao use separate detection controls that match their real backend capabilities, and both modal flows work correctly from the keyboard.

## Confirmed Product Rules

- Douyin detection supports Token, password, certification, aid, and registration-time options.
- Toutiao detection supports the existing login/certification check only.
- Douyin and Toutiao each receive a dedicated start button.
- A single stop action stops the currently running detection batch.
- This change reorganizes existing detection behavior; it does not invent unsupported Toutiao Token or password checks.

## Workbench Layout

### Responsive three-part shell

The workbench keeps the existing left-action, center-table, right-configuration structure. The left and right rails use bounded widths, while the center panel uses `minmax(0, 1fr)` so the three columns fit inside the default 1440px Tauri window. The page itself must not gain a horizontal scrollbar; only the table viewport may scroll horizontally.

At narrower widths, the configuration rail moves below the table. The two platform cards sit side by side when space permits and stack vertically at small widths.

### Left action rail

Buttons are divided into labeled groups:

1. List: clear list.
2. Export: export all, offline, and online rows.
3. Package allocation: allocate online, offline, normal-function, and limited-function ZIPs.

`清空列表框` becomes `清空列表`, uses destructive styling when data exists, and is disabled when there are no rows.

### Center table

- Status, APP, account, and primary result fields remain the most visually prominent.
- The first identity columns stay visible while the table scrolls horizontally.
- Row status uses a compact badge in addition to background color.
- The empty state contains a direct `扫描资源` action.
- Duration uses milliseconds below one second and seconds at or above one second.
- Existing exported CSV fields remain unchanged.

## Platform Detection Cards

The generic `检测 APP` selector is removed from the visible configuration rail and replaced by two always-visible platform cards.

### Douyin card

Controls:

- Token
- 密码
- 实名
- aid
- 注册时间
- `开始检测抖音`

Starting this card filters the batch rows to Douyin apps and passes the selected Douyin options to the existing detector.

### Toutiao card

Controls:

- 登录/实名状态
- `开始检测头条`

Starting this card filters the batch rows to Toutiao apps and only invokes the existing Toutiao certification/login-status path. Douyin-only options are never displayed or passed as meaningful Toutiao capabilities.

### Shared running state

Only one platform batch runs at a time. While detection is running, both start buttons and all platform controls are disabled. A shared `停止当前检测` button remains available. Each card shows the number of scanned apps it can operate on; its start button is disabled when that count is zero.

## Modal Interaction

Both scan and detail overlays become real modal dialogs:

- `role="dialog"` and `aria-modal="true"`
- labelled headings
- focus moves into the dialog when opened
- Tab and Shift+Tab remain inside the dialog
- Escape closes the dialog when closing is allowed
- closing restores focus to the control that opened the dialog
- background page scrolling is locked while a modal is open

Scan copy changes from implementation commentary to task-oriented language. `扫描路径` becomes `开始扫描`.

## State and Code Boundaries

- `BatchDetectionOptions` is reshaped into platform-specific option groups.
- Platform selection is supplied by the dedicated start action instead of a shared selector.
- Pure helpers determine platform-specific row filtering, supported controls, and duration formatting so they can be covered by Node tests.
- A reusable modal-focus helper or hook owns Escape handling, focus trapping, scroll locking, and focus restoration for both dialogs.
- Existing Tauri command implementations remain unchanged.

## Error and Empty States

- Starting a platform with no scanned matching apps remains impossible through a disabled button and visible zero count.
- Scan and detection errors continue to surface through the workbench status line.
- An unsuccessful scan keeps the scan dialog open.
- A running scan cannot be dismissed until the existing loading rule allows it.

## Testing and Verification

Automated tests cover:

- platform-specific options and row filtering
- Douyin and Toutiao start actions selecting the correct app type
- duration formatting
- modal Escape and focus-trap behavior where practical through extracted pure logic
- the existing queued React event-value regression

Verification includes:

- `npm test`
- `npm run build`
- Rust tests if frontend changes require a fresh packaged application build
- packaged macOS app inspection at the default window size
- main workbench, scan-dialog, and keyboard-interaction checks

## Out of Scope

- New Toutiao Token, password, aid, or registration-time backend checks
- Changes to CSV columns or backend result models
- A full table virtualization rewrite
- New visual assets or branding
