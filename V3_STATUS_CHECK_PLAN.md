# V3 Status Check Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在当前 Tauri 桌面项目中完成第三版“状态检测”能力，支持从 iOS 沙盒 ZIP 中自动提取抖音与今日头条登录态，并联网判断“抖音是否已设置密码”“头条是否已完成认证”；同时支持批量拖入多个 ZIP 文件后直接自动扫描。

**Architecture:** 继续复用第二版的“ZIP 扫描 -> 定位候选文件 -> 解析 Preferences / Cookies -> 前端展示”主链路，把第三版能力收敛为两个独立的 Tauri 后端命令，并在扫描入口层补齐“单 ZIP / 目录 / 多 ZIP 拖入”三种输入模式。后端负责从 ZIP 内自动提取参数并请求目标接口，前端在现有抖音/头条专属面板中增加状态卡片，同时通过 Tauri 原生拖拽事件把多个 ZIP 路径自动写入并触发扫描。

**Tech Stack:** Tauri 2、Rust、Reqwest blocking client、React、TypeScript、Vite、plist、zip

---

## 文件结构

- **Modify:** `src-tauri/src/lib.rs`
  - 继续承载 Tauri command、ZIP 读取、plist / Cookies 解析、联网请求逻辑
- **Modify:** `src/App.tsx`
  - 继续承载抖音/头条参数面板、状态请求、副作用管理、拖拽监听和检测结果展示
- **Create:** `V3_STATUS_CHECK_PLAN.md`
  - 记录第三版目标、数据来源、接口定义、任务拆解与验证方式

## 当前范围

- **抖音密码状态检测**
  - 接口：`https://api5-normal-lf.amemv.com/passport/account/info/v2/...`
  - 请求头：`x-ss-cookie: sessionid=...`
  - 核心字段：`data.has_password`
- **头条认证状态检测**
  - 接口：`https://webcast5-open-lf.douyin.com/webcast/openapi/certification/get_certification_status/?webcast_app_id=6822&aid=13`
  - 请求头：`authorization: Bearer <tt_acttoken>`、`Cookie: odin_tt=...`
  - 核心字段：`is_verified` / `data.is_verified`
- **批量拖入 ZIP 自动扫描**
  - 输入方式：把一个或多个 `.zip` 文件直接拖入桌面窗口
  - 前端行为：过滤非 ZIP 路径、写入输入框、自动触发扫描
  - 后端行为：支持把多行路径识别为“files”批量输入模式

## 设计原则

- 尽量复用第二版 ZIP 扫描和参数提取能力，不额外引入新页面或新存储层
- 第三版只做“状态检测”，不在本阶段接入“改密”“认证提交流程”
- 保持扫描入口统一，允许“单 ZIP”“目录扫描”“多 ZIP 拖入”最终都汇总到同一条扫描主链路
- 后端接口允许返回 `missing_cookie`、`missing_sessionid`、`missing_act_token`、`parse_error` 等可展示状态
- 前端优先展示“检测中 / 已设置 / 未设置 / 已认证 / 未认证 / 失败”，不暴露过多底层异常细节

## 数据来源

- **抖音**
  - Preferences：`Library/Preferences/com.ss.iphone.ugc.Aweme.plist`
  - Cookies：`Library/Cookies/Cookies.binarycookies`
  - 登录态来源：从 Cookies 中提取 `sessionid`
- **今日头条**
  - Preferences：`Library/Preferences/com.ss.iphone.article.news.plist`
  - Cookies：`Library/Cookies/Cookies.binarycookies`
  - 接口参数来源：
    - `act_token` 来自 plist 的 `bdaccount_session_x_tt_token`
    - `odin_tt` 来自 Cookies

## 接口输出约定

- **抖音检测结果**
  - `sourceZip`
  - `sourceCookiePath`
  - `sessionId`
  - `hasPassword`
  - `accountName`
  - `status`
  - `error`
- **头条检测结果**
  - `sourceZip`
  - `sourcePlistPath`
  - `sourceCookiePath`
  - `actToken`
  - `odinTt`
  - `isVerified`
  - `status`
  - `error`

## 扫描入口约定

- **单 ZIP**
  - 输入：一个 zip 文件绝对路径
  - `sourceMode`：`zip`
- **目录扫描**
  - 输入：一个目录绝对路径
  - 行为：递归搜集目录下全部 zip
  - `sourceMode`：`directory`
- **批量拖入**
  - 输入：多行文本，每行一个 zip 绝对路径
  - 行为：逐行校验是否为 zip 文件，全部通过后批量扫描
  - `sourceMode`：`files`
- **前端拖拽体验**
  - 拖入窗口时高亮输入区域
  - 仅接受 `.zip` 文件
  - 拖入成功后自动调用扫描，不要求再点按钮

## 已落地的最小版本

- 已新增两个 Tauri command：
  - `check_douyin_password_status`
  - `check_toutiao_certification_status`
- 已完成前端最小接入：
  - 抖音参数面板显示密码状态
  - 头条参数面板显示认证状态
- 已完成批量拖入 ZIP 接入：
  - 支持拖入一个或多个 ZIP 后自动扫描
  - 输入区支持拖拽高亮提示
  - 扫描摘要可区分 `zip` / `directory` / `files`
- 已完成基础单测：
  - Cookie 键提取
  - `has_password` 解析
  - `is_verified` 解析
  - 多行 ZIP 路径解析为批量输入

---

### Task 1: 固化第三版后端状态结构

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写失败测试，覆盖状态字段解析**

```rust
#[test]
fn parses_douyin_password_status_from_data_field() {
    let payload = json!({
        "data": {
            "has_password": 1,
            "screen_name": "demo_user"
        }
    });

    let status = parse_douyin_password_status_payload(&payload);

    assert_eq!(status.has_password, Some(true));
    assert_eq!(status.screen_name.as_deref(), Some("demo_user"));
}

#[test]
fn parses_toutiao_certification_status_from_nested_data() {
    let payload = json!({
        "data": {
            "is_verified": true
        }
    });

    let status = parse_toutiao_certification_status_payload(&payload);

    assert_eq!(status.is_verified, Some(true));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test`
Expected: 失败，提示缺少 `parse_douyin_password_status_payload` 或 `parse_toutiao_certification_status_payload`

- [ ] **Step 3: 写最小实现**

```rust
#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedDouyinPasswordStatus {
    has_password: Option<bool>,
    screen_name: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedToutiaoCertificationStatus {
    is_verified: Option<bool>,
}

fn parse_douyin_password_status_payload(payload: &Value) -> ParsedDouyinPasswordStatus {
    let data = payload.get("data").unwrap_or(payload);
    let has_password = data.get("has_password").and_then(normalize_password_value);
    let screen_name = first_non_empty_strings(&[
        data.get("name").and_then(douyin_normalize_json_value),
        data.get("screen_name").and_then(douyin_normalize_json_value),
    ]);

    ParsedDouyinPasswordStatus {
        has_password,
        screen_name,
    }
}

fn parse_toutiao_certification_status_payload(payload: &Value) -> ParsedToutiaoCertificationStatus {
    let is_verified = payload
        .get("data")
        .and_then(|value| value.get("is_verified"))
        .or_else(|| payload.get("is_verified"))
        .and_then(normalize_boolish_value);

    ParsedToutiaoCertificationStatus { is_verified }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add status payload parsers"
```

### Task 2: 补全 Cookie 参数提取能力

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写失败测试，覆盖 Cookie 头提取**

```rust
#[test]
fn reads_cookie_value_from_joined_header() {
    let cookie_header = "sessionid=abc123; odin_tt=odin_value; passport_csrf_token=token";

    assert_eq!(
        extract_cookie_value(cookie_header, "sessionid").as_deref(),
        Some("abc123")
    );
    assert_eq!(
        extract_cookie_value(cookie_header, "odin_tt").as_deref(),
        Some("odin_value")
    );
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test`
Expected: 失败，提示缺少 `extract_cookie_value`

- [ ] **Step 3: 写最小实现**

```rust
fn extract_cookie_value(cookie_header: &str, key: &str) -> Option<String> {
    let target = format!("{key}=");
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix(&target) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add cookie value extractor"
```

### Task 3: 接入抖音密码状态检测命令

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写失败测试，锁定抖音状态结构**

```rust
#[test]
fn parses_douyin_password_status_from_data_field() {
    let payload = json!({
        "data": {
            "has_password": 1,
            "screen_name": "demo_user"
        }
    });

    let status = parse_douyin_password_status_payload(&payload);

    assert_eq!(status.has_password, Some(true));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test`
Expected: FAIL

- [ ] **Step 3: 写最小实现**

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinPasswordStatusResult {
    source_zip: String,
    source_cookie_path: Option<String>,
    session_id: String,
    has_password: Option<bool>,
    account_name: Option<String>,
    status: String,
    error: Option<String>,
}

#[tauri::command]
fn check_douyin_password_status(zip_path: String) -> Result<DouyinPasswordStatusResult, String> {
    // 1. 找 Cookies.binarycookies
    // 2. 解析 cookieHeader
    // 3. 提取 sessionid
    // 4. 请求 account/info/v2
    // 5. 解析 has_password
    unimplemented!()
}
```

- [ ] **Step 4: 注册命令并确认通过**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add douyin password status command"
```

### Task 4: 接入头条认证状态检测命令

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写失败测试，锁定头条认证字段**

```rust
#[test]
fn parses_toutiao_certification_status_from_nested_data() {
    let payload = json!({
        "data": {
            "is_verified": true
        }
    });

    let status = parse_toutiao_certification_status_payload(&payload);

    assert_eq!(status.is_verified, Some(true));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test`
Expected: FAIL

- [ ] **Step 3: 写最小实现**

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToutiaoCertificationStatusResult {
    source_zip: String,
    source_plist_path: Option<String>,
    source_cookie_path: Option<String>,
    act_token: String,
    odin_tt: String,
    is_verified: Option<bool>,
    status: String,
    error: Option<String>,
}

#[tauri::command]
fn check_toutiao_certification_status(zip_path: String) -> Result<ToutiaoCertificationStatusResult, String> {
    // 1. 找今日头条 plist
    // 2. 从 plist 提 act_token
    // 3. 从 Cookies 提 odin_tt
    // 4. 请求 get_certification_status
    // 5. 解析 is_verified
    unimplemented!()
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add toutiao certification status command"
```

### Task 5: 在前端参数面板接入第三版检测结果

**Files:**
- Modify: `src/App.tsx`
- Test: 手工验证

- [ ] **Step 1: 新增前端结果类型**

```ts
type DouyinPasswordStatusResult = {
  sourceZip: string;
  sourceCookiePath?: string | null;
  sessionId: string;
  hasPassword?: boolean | null;
  accountName?: string | null;
  status: string;
  error?: string | null;
};

type ToutiaoCertificationStatusResult = {
  sourceZip: string;
  sourcePlistPath?: string | null;
  sourceCookiePath?: string | null;
  actToken: string;
  odinTt: string;
  isVerified?: boolean | null;
  status: string;
  error?: string | null;
};
```

- [ ] **Step 2: 在抖音和头条专属面板增加副作用请求**

```ts
void invoke<DouyinPasswordStatusResult>("check_douyin_password_status", {
  zipPath: selectedFile.sourceZip,
});

void invoke<ToutiaoCertificationStatusResult>("check_toutiao_certification_status", {
  zipPath: selectedFile.sourceZip,
});
```

- [ ] **Step 3: 在 UI 中展示状态卡片**

```tsx
<article className="special-summary-card">
  <span>密码状态</span>
  <strong>{formatDouyinPasswordLabel(douyinPasswordStatus)}</strong>
</article>

<article className="special-summary-card">
  <span>认证状态</span>
  <strong>{formatToutiaoCertificationLabel(toutiaoCertificationStatus)}</strong>
</article>
```

- [ ] **Step 4: 构建验证**

Run: `npm run build`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/App.tsx
git commit -m "feat: show v3 status checks in app panels"
```

### Task 6: 接入批量拖入 ZIP 自动扫描

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写失败测试，覆盖多行 ZIP 路径输入**

```rust
#[test]
fn resolves_multiple_zip_paths_from_multiline_input() {
    let temp_dir = tempdir().expect("tempdir");
    let zip_a = temp_dir.path().join("a.zip");
    let zip_b = temp_dir.path().join("b.zip");
    fs::write(&zip_a, b"").expect("zip_a");
    fs::write(&zip_b, b"").expect("zip_b");

    let input = format!(
        "{}\n{}",
        zip_a.to_string_lossy(),
        zip_b.to_string_lossy()
    );

    let scan_input = resolve_scan_input(&input).expect("scan_input");

    assert_eq!(scan_input.source_mode, "files");
    assert_eq!(
        scan_input.zip_paths,
        vec![
            zip_a.to_string_lossy().to_string(),
            zip_b.to_string_lossy().to_string()
        ]
    );
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test resolves_multiple_zip_paths_from_multiline_input`
Expected: FAIL，提示 `resolve_scan_input` 还不支持多行 zip 路径

- [ ] **Step 3: 写最小后端实现**

```rust
fn resolve_scan_input(input_path: &str) -> Result<ScanInput, String> {
    let manual_zip_paths = extract_manual_zip_paths(input_path);
    if manual_zip_paths.len() > 1 {
        let mut zip_paths = Vec::with_capacity(manual_zip_paths.len());
        for zip_path in manual_zip_paths {
            let path = Path::new(&zip_path);
            let metadata =
                fs::metadata(path).map_err(|error| format!("path_stat_failed: {error}"))?;
            if !metadata.is_file() || !is_zip_file(path) {
                return Err("scan_path_failed: 拖入的项目里包含非 zip 文件".to_string());
            }
            zip_paths.push(path.to_string_lossy().to_string());
        }

        return Ok(ScanInput {
            source_mode: "files".to_string(),
            zip_paths,
        });
    }

    // 原有单 zip / 目录逻辑保持不变
}
```

- [ ] **Step 4: 写最小前端实现**

```ts
void getCurrentWebview()
  .onDragDropEvent((event) => {
    if (event.payload.type === "enter" || event.payload.type === "over") {
      setIsDragOver(true);
      return;
    }

    if (event.payload.type === "leave") {
      setIsDragOver(false);
      return;
    }

    const droppedZipPaths = filterZipDropPaths(event.payload.paths);
    setIsDragOver(false);

    if (!droppedZipPaths.length) {
      setStatus("拖入失败：仅支持 ZIP 文件");
      return;
    }

    const nextSourcePath = droppedZipPaths.join("\n");
    setSourcePath(nextSourcePath);
    void handleScan(nextSourcePath);
  });
```

- [ ] **Step 5: 运行验证并提交**

Run:

```bash
cargo test resolves_multiple_zip_paths_from_multiline_input
npm run build
```

Expected:
- Rust 测试通过
- 前端构建通过
- 拖入多个 ZIP 后自动开始扫描

```bash
git add src-tauri/src/lib.rs src/App.tsx src/App.css
git commit -m "feat: support drag and drop zip batch scanning"
```

### Task 7: 做整体验证与回归检查

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/App.tsx`

- [ ] **Step 1: 运行 Rust 测试**

Run: `cargo test`
Expected: 所有测试通过

- [ ] **Step 2: 运行前端构建**

Run: `npm run build`
Expected: 构建通过，无 TypeScript 错误

- [ ] **Step 3: 检查编辑文件诊断**

Run:

```text
GetDiagnostics(file:///Users/edking/Documents/网赚学习/ios_zen_plist_read/src/App.tsx)
GetDiagnostics(file:///Users/edking/Documents/网赚学习/ios_zen_plist_read/src-tauri/src/lib.rs)
```

Expected: 诊断为空

- [ ] **Step 4: 用真实 ZIP 手工验证**

Run:

```bash
npm run tauri dev
```

Expected:
- 抖音面板能看到“密码状态”
- 头条面板能看到“认证状态”
- 拖入多个 ZIP 后能自动批量扫描
- 扫描模式会显示 `files`
- 缺少登录态时显示失败或缺参状态，不崩溃

- [ ] **Step 5: 提交**

```bash
git add src/App.tsx src-tauri/src/lib.rs V3_STATUS_CHECK_PLAN.md
git commit -m "docs: add v3 status check plan"
```

---

## 后续扩展

- 支持头条 SQLite Cookie 兜底，不只依赖 `Cookies.binarycookies`
- 增加“导出状态检测结果”按钮
- 首页标题从“第二版原型”升级到“第三版原型”
- 把第三版状态检测抽成独立导出 JSON 结构，方便批量处理
- 为联网请求加本地缓存，避免重复点击时频繁请求外部接口
- 拖入 ZIP 后把输入框展示优化成“已选择 N 个 ZIP”的摘要模式

## 交付标准

- 用户选中抖音 ZIP 后，可看到“是否已设置密码”
- 用户选中头条 ZIP 后，可看到“是否已认证”
- 用户拖入一个或多个 ZIP 后，可自动开始扫描并得到结果
- 参数缺失、接口失败、字段缺失时，界面能稳定展示错误状态
- `cargo test` 和 `npm run build` 均通过

Plan complete and saved to `V3_STATUS_CHECK_PLAN.md`. Two execution options:

1. Subagent-Driven (recommended) - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. Inline Execution - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
