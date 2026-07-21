# 今日头条 Token 检测 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用每个 iOS 沙盒 ZIP 内的真实头条凭据调用 `tabs_api/v1`，判断 Token 状态并填充用户名、UID 和注册时间。

**Architecture:** Rust/Tauri 后端负责从头条 plist 与 binarycookies 中选择当前凭据、发起网络请求、解析业务响应，并只返回掩码后的秘密值。React 批量检测沿用现有 `BatchDetectionRow`，通过独立的头条 Token 选项调用新命令并合并行级状态。

**Tech Stack:** Rust 2021、Tauri 2、reqwest blocking、plist、serde_json、React 19、TypeScript 5、Node test runner。

## Global Constraints

- Token 和现有“登录/实名状态”选项必须独立且默认开启。
- 固定请求参数为 `app_name=news_article`、`aid=13`、`detail=my_tabs_v2`、`user_app_id=1128`。
- 必须从当前 ZIP 提取 Token、Cookie、`device_id` 和 `iid`，不得写死示例凭据或设备值。
- 完整 Token、Cookie 和请求头不得进入前端、CSV、错误信息或测试快照。
- 只有业务成功且存在有效 `profile.data.user_id` 才能把 Token 判定为在线。
- 当前目录没有 `.git`，各任务完成后运行验证并记录检查点，不执行无法完成的 Git commit。

---

### Task 1: 独立的头条 Token 检测选项

**Files:**
- Modify: `src/batch-options.ts`
- Test: `tests/batch-options.test.ts`

**Interfaces:**
- Consumes: `buildBatchDetectionOptions(platform, douyin, toutiao)`。
- Produces: `ToutiaoDetectionOptions = { token: boolean; certification: boolean }`；头条运行返回 `token: toutiao.token`。

- [ ] **Step 1: 写入失败测试**

将测试中的头条配置改为：

```ts
const toutiaoOptions: ToutiaoDetectionOptions = {
  token: true,
  certification: true,
};
```

并断言头条构造结果含 `token: true`，同时新增一个 `token: false` 的隔离用例，证明头条选项不会读取 `douyinOptions.token`。

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- --test-name-pattern="Toutiao"`

Expected: TypeScript 或断言失败，因为 `ToutiaoDetectionOptions` 尚无 `token`，且头条运行仍固定 `token: false`。

- [ ] **Step 3: 实现最小选项变更**

```ts
export type ToutiaoDetectionOptions = {
  token: boolean;
  certification: boolean;
};

// buildBatchDetectionOptions 的头条分支
return {
  appType: platform,
  token: toutiao.token,
  password: false,
  certification: toutiao.certification,
  aid: false,
  registrationTime: false,
};
```

- [ ] **Step 4: 运行测试确认通过**

Run: `npm test -- --test-name-pattern="Toutiao"`

Expected: 所有匹配的头条选项测试通过。

### Task 2: 头条参数选择和响应解析

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `serde_json::Value`、现有 `parse_binarycookies_bytes`、`mask_secret`、`douyin_normalize_json_value`。
- Produces: `ToutiaoTokenStatusResult`、`ParsedToutiaoTokenCheck`、`toutiao_token_value`、`toutiao_device_id`、`toutiao_cookie_value`、`parse_toutiao_token_payload`。

- [ ] **Step 1: 添加失败的 Rust 单元测试**

增加覆盖以下行为的测试：

```rust
#[test]
fn parses_successful_toutiao_token_payload() {
    let payload = json!({
        "message": "success",
        "profile": {
            "errno": 0,
            "message": "success",
            "data": {
                "name": "测试用户",
                "user_id": 819616220453017_u64,
                "create_time": "1778145951"
            }
        }
    });
    let parsed = parse_toutiao_token_payload(&payload);
    assert_eq!(parsed.is_valid, Some(true));
    assert_eq!(parsed.nickname.as_deref(), Some("测试用户"));
    assert_eq!(parsed.uid.as_deref(), Some("819616220453017"));
    assert_eq!(parsed.register_time.as_deref(), Some("1778145951"));
}
```

另加业务失败、成功响应缺 UID、`FlowSaveDeviceId.deviceId` 回退到 `kOldDeviceIDStorageKey`、Token 首选 `kTTAccountTokenGuardXTTToken`、Cookie 同名时选择最新非空值的测试。

- [ ] **Step 2: 运行 Rust 测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml toutiao_token -- --nocapture`

Expected: FAIL，提示新类型或函数不存在。

- [ ] **Step 3: 实现纯解析与选择函数**

新增结果字段：

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToutiaoTokenStatusResult {
    source_zip: String,
    source_plist_path: Option<String>,
    source_cookie_path: Option<String>,
    token_preview: String,
    odin_tt_preview: String,
    device_id: String,
    iid: String,
    nickname: Option<String>,
    uid: Option<String>,
    register_time: Option<String>,
    http_status: Option<u16>,
    status: String,
    error: Option<String>,
}
```

`parse_toutiao_token_payload` 必须把数值或字符串 UID 统一成字符串；顶层和 `profile` 均成功但缺 UID 时返回 `is_valid: None`，明确业务失败时返回 `Some(false)`。

`toutiao_cookie_value` 从 `parsed_cookies.cookies` 中按名称过滤非空值，优先 `.toutiaoapi.com`、`.toutiao.com`、`.snssdk.com` 域，再按 `created` 最大值选择；没有结构化 Cookie 时才回退现有 `cookieHeader`。

- [ ] **Step 4: 运行 Rust 解析测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml toutiao_token -- --nocapture`

Expected: 新增纯函数测试全部通过，无网络请求。

### Task 3: ZIP 凭据提取和 Token 检测命令

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 2 的选择/解析函数，现有 `find_app_file_path`、`read_zip_entry_bytes`。
- Produces: Tauri 命令 `check_toutiao_token_status(zip_path: String) -> Result<ToutiaoTokenStatusResult, String>`。

- [ ] **Step 1: 添加 ZIP 提取测试**

新增环境变量驱动的 ignored 测试，不把 Downloads 路径写入源码：

```rust
#[test]
#[ignore = "requires TOUTIAO_TOKEN_TEST_ZIP and live network"]
fn checks_toutiao_token_live_fixture() {
    let zip_path = std::env::var("TOUTIAO_TOKEN_TEST_ZIP").expect("fixture path");
    let result = check_toutiao_token_status_impl(zip_path).expect("token check");
    println!(
        "status={} uid={} nickname={} register_time={}",
        result.status,
        result.uid.as_deref().unwrap_or("-"),
        result.nickname.as_deref().unwrap_or("-"),
        result.register_time.as_deref().unwrap_or("-")
    );
    assert!(!result.device_id.is_empty());
    assert!(!result.iid.is_empty());
    assert!(!result.token_preview.contains("--"));
}
```

- [ ] **Step 2: 实现命令和请求**

实现顺序：查找头条 plist/Cookies、读取并解析、校验四个必需参数、构造白名单 Cookie 和 User-Agent、发送 15 秒超时 GET、解析 JSON。查询参数通过 reqwest `.query(&[(key, value), ...])` 生成，Header 只使用掩码之外的内部值；错误字符串只包含错误类别和 HTTP 状态。

状态映射：`Some(true) -> ok`、`Some(false) -> invalid`、`None -> parse_error`；缺失参数分别返回 `missing_token`、`missing_odin_tt`、`missing_device_id`、`missing_iid`。

- [ ] **Step 3: 注册 Tauri 命令**

把 `check_toutiao_token_status` 加入 `tauri::generate_handler!`，确保前端可通过同名 snake_case 命令调用。

- [ ] **Step 4: 运行后端测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: 全部非 ignored 测试通过，live fixture 测试显示 ignored。

### Task 4: React 批量检测接入

**Files:**
- Modify: `src/App.tsx`
- Test: `tests/batch-options.test.ts`

**Interfaces:**
- Consumes: `ToutiaoTokenStatusResult` camelCase JSON 和 `ToutiaoDetectionOptions.token`。
- Produces: 头条卡片 Token 复选框、`formatToutiaoTokenLabel`、头条行身份字段与状态合并。

- [ ] **Step 1: 定义前端结果类型和默认状态**

```ts
type ToutiaoTokenStatusResult = {
  sourceZip: string;
  sourcePlistPath?: string | null;
  sourceCookiePath?: string | null;
  tokenPreview: string;
  odinTtPreview: string;
  deviceId: string;
  iid: string;
  nickname?: string | null;
  uid?: string | null;
  registerTime?: string | null;
  httpStatus?: number | null;
  status: string;
  error?: string | null;
};
```

初始化 `toutiaoOptions` 为 `{ token: true, certification: true }`。

- [ ] **Step 2: 添加头条 Token 控件**

在今日头条卡片中增加绑定 `toutiaoOptions.token` 的复选框，使用现有 `queueBatchOptionFromEvent`，保持 React queued-event 安全模式。

- [ ] **Step 3: 接入批量检测结果**

在头条分支中，勾选 Token 时调用：

```ts
const tokenResult = await invoke<ToutiaoTokenStatusResult>(
  "check_toutiao_token_status",
  { zipPath: row.sourceZip },
);
```

成功时把 `nickname`、`uid`、`formatRegisterTime(registerTime)` 写回行字段，设置 `onlineSignal = true` 和“Token 在线”；`invalid` 设置 `offlineSignal = true`；缺参数、HTTP、请求和解析错误只写入错误与 Token 状态，不伪造掉线。`fullParams` 只展示 `device_id`、`iid` 和四个固定业务参数。

- [ ] **Step 4: 运行前端测试和构建**

Run: `npm test`

Expected: 所有 Node 测试通过。

Run: `npm run build`

Expected: TypeScript 和 Vite 构建成功。

### Task 5: 两个真实 ZIP 与打包应用验收

**Files:**
- Modify only if validation reveals defects: `src-tauri/src/lib.rs`, `src/App.tsx`, `src/batch-options.ts`, related tests

**Interfaces:**
- Consumes: 完整 Token 检测链路。
- Produces: 两个包的真实状态证据和可运行 `.app`。

- [ ] **Step 1: 验证第一个 ZIP**

Run:

```bash
TOUTIAO_TOKEN_TEST_ZIP=/Users/edking/Downloads/20260610-05-23-53_62995022752.zip \
cargo test --manifest-path src-tauri/Cargo.toml checks_toutiao_token_live_fixture -- --ignored --nocapture
```

Expected: 能提取非空 `device_id`/`iid`；输出只含状态、UID、用户名和注册时间，不泄露 Token/Cookie。

- [ ] **Step 2: 验证第二个 ZIP**

Run:

```bash
TOUTIAO_TOKEN_TEST_ZIP=/Users/edking/Downloads/20260610-05-33-22_29159868055.zip \
cargo test --manifest-path src-tauri/Cargo.toml checks_toutiao_token_live_fixture -- --ignored --nocapture
```

Expected: 使用第二个包自己的参数并输出真实结果；若 Token 已过期，准确显示 `invalid` 而不是测试失败。

- [ ] **Step 3: 运行完整静态验证**

Run: `npm test && npm run build && cargo test --manifest-path src-tauri/Cargo.toml && cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

Expected: 所有非网络测试、构建和格式检查通过。

- [ ] **Step 4: 构建 macOS 应用**

Run: `npm run tauri build -- --bundles app`

Expected: 生成 `src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app`。

- [ ] **Step 5: 检查产物**

确认 `.app` 存在，记录路径和构建时间；在可用的桌面环境中打开打包应用，扫描测试 ZIP，核对头条 Token 复选框、状态、用户名、UID、注册时间和错误详情。
