# 抖音与头条在线包分开分配 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将统一的在线 ZIP 分配拆成抖音与头条两个独立入口，平台独有 ZIP 移动到对应目录，双平台在线 ZIP 复制到两个目录并保留源文件。

**Architecture:** 前端纯函数根据全部批量行计算当前平台的 `movePaths` 与 `copyPaths`，React 只负责编排 Tauri 调用和状态反馈。Rust 后端通过共享的受限 ZIP 传输实现提供移动与复制命令，并用固定白名单限制目标子目录。

**Tech Stack:** React 19、TypeScript 5.8、Node test runner、Tauri 2.8、Rust、`std::fs`

## Global Constraints

- 抖音目标目录固定为 `douyin_online/`，头条目标目录固定为 `toutiao_online/`。
- 只属于一个在线平台的 ZIP 使用移动；同时属于两个在线平台的 ZIP 使用复制并保留源 ZIP。
- 分配按 `sourceZip` 去重，多账号行不能重复处理同一个 ZIP。
- 掉线包、正常功能包和限制功能包保持现状。
- 不修改抖音或头条在线状态判定逻辑。
- 不覆盖目标目录中已存在的同名 ZIP。
- 目标目录必须经过后端固定白名单验证，不接受调用方提供的任意路径。
- 当前工作目录没有 `.git`，无法创建提交；每个任务使用测试结果和文件检查作为阶段检查点。

---

## File Structure

- Create `src/package-allocation.ts`: 纯平台在线 ZIP 集合与移动/复制计划生成。
- Create `tests/package-allocation.test.ts`: 分配计划、去重与双平台交集回归测试。
- Modify `src/App.tsx`: 两个平台按钮、独立运行状态、Tauri 移动/复制编排。
- Modify `tests/workbench-layout.test.ts`: 工作台源码契约，锁定两个按钮和复制命令接入。
- Modify `src-tauri/src/lib.rs`: 共享 ZIP 传输实现、`copy_zip_files` 命令、白名单与 Rust 测试。

---

### Task 1: 纯在线包分配计划

**Files:**
- Create: `src/package-allocation.ts`
- Create: `tests/package-allocation.test.ts`

**Interfaces:**
- Consumes: `DetectionPlatform` from `src/batch-options.ts`；批量行的 `sourceZip`、`appType`、`status`。
- Produces: `buildOnlineAllocationPlan(rows, platform): OnlinePackageAllocationPlan`，返回 `targetSubdir`、`movePaths`、`copyPaths`。

- [ ] **Step 1: 写失败测试，锁定平台筛选、去重与双平台复制**

Create `tests/package-allocation.test.ts`:

```ts
import assert from "node:assert/strict";
import test from "node:test";
import { buildOnlineAllocationPlan } from "../src/package-allocation.ts";

const rows = [
  { sourceZip: "/batch/douyin.zip", appType: "douyin" as const, status: "online" },
  { sourceZip: "/batch/douyin.zip", appType: "douyin" as const, status: "online" },
  { sourceZip: "/batch/toutiao.zip", appType: "toutiao" as const, status: "online" },
  { sourceZip: "/batch/shared.zip", appType: "douyin" as const, status: "online" },
  { sourceZip: "/batch/shared.zip", appType: "toutiao" as const, status: "online" },
  { sourceZip: "/batch/offline.zip", appType: "douyin" as const, status: "offline" },
  { sourceZip: "/batch/failed.zip", appType: "toutiao" as const, status: "failed" },
];

test("builds the Douyin online allocation plan", () => {
  assert.deepEqual(buildOnlineAllocationPlan(rows, "douyin"), {
    targetSubdir: "douyin_online",
    movePaths: ["/batch/douyin.zip"],
    copyPaths: ["/batch/shared.zip"],
  });
});

test("builds the Toutiao online allocation plan", () => {
  assert.deepEqual(buildOnlineAllocationPlan(rows, "toutiao"), {
    targetSubdir: "toutiao_online",
    movePaths: ["/batch/toutiao.zip"],
    copyPaths: ["/batch/shared.zip"],
  });
});

test("ignores blank paths and non-online rows", () => {
  assert.deepEqual(buildOnlineAllocationPlan([
    { sourceZip: "", appType: "douyin", status: "online" },
    { sourceZip: "  ", appType: "toutiao", status: "online" },
    { sourceZip: "/batch/pending.zip", appType: "douyin", status: "pending" },
    { sourceZip: "/batch/skipped.zip", appType: "toutiao", status: "skipped" },
  ], "douyin"), {
    targetSubdir: "douyin_online",
    movePaths: [],
    copyPaths: [],
  });
});
```

- [ ] **Step 2: 运行测试并确认因模块缺失而失败**

Run: `node --test tests/package-allocation.test.ts`

Expected: FAIL，错误包含无法找到 `src/package-allocation.ts`。

- [ ] **Step 3: 实现最小纯分配器**

Create `src/package-allocation.ts`:

```ts
import type { DetectionPlatform } from "./batch-options";

export type OnlineAllocationRow = {
  sourceZip: string;
  appType: DetectionPlatform;
  status: string;
};

export type OnlineTargetSubdir = "douyin_online" | "toutiao_online";

export type OnlinePackageAllocationPlan = {
  targetSubdir: OnlineTargetSubdir;
  movePaths: string[];
  copyPaths: string[];
};

function collectOnlinePaths(rows: OnlineAllocationRow[], platform: DetectionPlatform) {
  const paths = new Set<string>();
  for (const row of rows) {
    const sourceZip = row.sourceZip.trim();
    if (row.appType === platform && row.status === "online" && sourceZip) {
      paths.add(sourceZip);
    }
  }
  return paths;
}

export function buildOnlineAllocationPlan(
  rows: OnlineAllocationRow[],
  platform: DetectionPlatform,
): OnlinePackageAllocationPlan {
  const otherPlatform: DetectionPlatform = platform === "douyin" ? "toutiao" : "douyin";
  const platformPaths = collectOnlinePaths(rows, platform);
  const otherPlatformPaths = collectOnlinePaths(rows, otherPlatform);
  const movePaths: string[] = [];
  const copyPaths: string[] = [];

  for (const sourceZip of platformPaths) {
    if (otherPlatformPaths.has(sourceZip)) copyPaths.push(sourceZip);
    else movePaths.push(sourceZip);
  }

  return {
    targetSubdir: platform === "douyin" ? "douyin_online" : "toutiao_online",
    movePaths,
    copyPaths,
  };
}
```

- [ ] **Step 4: 运行专项测试并确认通过**

Run: `node --test tests/package-allocation.test.ts`

Expected: 3 tests PASS，0 FAIL。

- [ ] **Step 5: 运行全部前端测试作为阶段检查点**

Run: `npm test`

Expected: 所有测试 PASS，0 FAIL。

---

### Task 2: Rust 受限复制与移动命令

**Files:**
- Modify: `src-tauri/src/lib.rs:798-879`
- Modify: `src-tauri/src/lib.rs:7164-7181`
- Test: `src-tauri/src/lib.rs` 的现有 `tests` 模块

**Interfaces:**
- Consumes: `Vec<String>` ZIP 路径和白名单目标目录字符串。
- Produces: Tauri 命令 `copy_zip_files(zip_paths: Vec<String>, target_subdir: String) -> Result<Vec<String>, String>`；保留同签名的 `move_zip_files`。

- [ ] **Step 1: 写失败测试，锁定新目录、复制保留源文件与禁止覆盖**

在 `src-tauri/src/lib.rs` 的 `tests` 模块加入：

```rust
#[test]
fn allows_only_known_package_target_directories() {
    assert!(is_allowed_zip_target_subdir("online"));
    assert!(is_allowed_zip_target_subdir("douyin_online"));
    assert!(is_allowed_zip_target_subdir("toutiao_online"));
    assert!(!is_allowed_zip_target_subdir("../outside"));
    assert!(!is_allowed_zip_target_subdir("custom"));
}

#[test]
fn copies_zip_without_removing_source() {
    let temp_dir = tempdir().expect("tempdir");
    let source = temp_dir.path().join("shared.zip");
    fs::write(&source, b"zip-content").expect("source zip");

    let result = copy_zip_files_impl(
        vec![source.to_string_lossy().to_string()],
        "douyin_online".to_string(),
    )
    .expect("copy result");

    let destination = temp_dir.path().join("douyin_online/shared.zip");
    assert!(source.is_file());
    assert_eq!(fs::read(destination).expect("copied zip"), b"zip-content");
    assert!(result[0].contains("成功复制 1 个文件"));
}

#[test]
fn copy_zip_does_not_overwrite_existing_destination() {
    let temp_dir = tempdir().expect("tempdir");
    let source = temp_dir.path().join("shared.zip");
    let destination_dir = temp_dir.path().join("toutiao_online");
    let destination = destination_dir.join("shared.zip");
    fs::write(&source, b"source").expect("source zip");
    fs::create_dir_all(&destination_dir).expect("destination dir");
    fs::write(&destination, b"existing").expect("existing zip");

    let error = copy_zip_files_impl(
        vec![source.to_string_lossy().to_string()],
        "toutiao_online".to_string(),
    )
    .expect_err("existing destination must fail");

    assert!(error.contains("目标文件已存在"));
    assert_eq!(fs::read(destination).expect("existing zip"), b"existing");
}

#[test]
fn copy_zip_rejects_non_zip_source() {
    let temp_dir = tempdir().expect("tempdir");
    let source = temp_dir.path().join("notes.txt");
    fs::write(&source, b"not a zip").expect("text file");

    let error = copy_zip_files_impl(
        vec![source.to_string_lossy().to_string()],
        "douyin_online".to_string(),
    )
    .expect_err("non-zip source must fail");

    assert!(error.contains("仅支持移动或复制 ZIP 文件"));
}
```

- [ ] **Step 2: 运行专项测试并确认编译失败来自缺少新接口**

Run: `cargo test --manifest-path src-tauri/Cargo.toml zip_ -- --nocapture`

Expected: FAIL，编译错误指出 `is_allowed_zip_target_subdir` 或 `copy_zip_files_impl` 不存在。

- [ ] **Step 3: 提取共享传输实现并新增复制命令**

用以下结构替换现有内联 `move_zip_files` 实现；文件遍历和错误汇总代码必须完整保留：

```rust
const ALLOWED_ZIP_TARGET_SUBDIRS: &[&str] = &[
    "online",
    "offline",
    "normal_functions",
    "limited_functions",
    "douyin_online",
    "toutiao_online",
];

#[derive(Clone, Copy)]
enum ZipTransferMode {
    Move,
    Copy,
}

fn is_allowed_zip_target_subdir(target_subdir: &str) -> bool {
    ALLOWED_ZIP_TARGET_SUBDIRS.contains(&target_subdir)
}

fn transfer_zip_files_impl(
    zip_paths: Vec<String>,
    target_subdir: String,
    mode: ZipTransferMode,
) -> Result<Vec<String>, String> {
    if !is_allowed_zip_target_subdir(&target_subdir) {
        return Err(format!("无效目标目录: {target_subdir}"));
    }

    let action = match mode {
        ZipTransferMode::Move => "移动",
        ZipTransferMode::Copy => "复制",
    };
    let mut transferred = Vec::new();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();

    for zip_path in &zip_paths {
        if !seen.insert(zip_path.clone()) {
            continue;
        }
        let src = Path::new(zip_path);
        if !src.is_file() {
            errors.push(format!("源文件不存在或不是文件: {zip_path}"));
            continue;
        }
        if !src
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("zip"))
        {
            errors.push(format!("仅支持移动或复制 ZIP 文件: {zip_path}"));
            continue;
        }
        let Some(parent) = src.parent() else {
            errors.push(format!("无法获取父目录: {zip_path}"));
            continue;
        };
        let filename = src
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.zip");
        let destination_dir = parent.join(&target_subdir);
        if let Err(error) = fs::create_dir_all(&destination_dir) {
            errors.push(format!(
                "创建目录失败 {}: {error}",
                destination_dir.display()
            ));
            continue;
        }
        let destination = destination_dir.join(filename);
        if destination.exists() {
            errors.push(format!("目标文件已存在: {}", destination.display()));
            continue;
        }

        let transfer_result = match mode {
            ZipTransferMode::Move => fs::rename(src, &destination),
            ZipTransferMode::Copy => fs::copy(src, &destination).map(|_| ()),
        };
        match transfer_result {
            Ok(()) => transferred.push(destination.display().to_string()),
            Err(error) => errors.push(format!(
                "{action}失败 {} -> {}: {error}",
                src.display(),
                destination.display()
            )),
        }
    }

    if transferred.is_empty() && !errors.is_empty() {
        Err(errors.join("；"))
    } else if !errors.is_empty() {
        Ok(vec![format!(
            "{action} {} 个文件成功，{} 个失败：{}",
            transferred.len(),
            errors.len(),
            errors.join("；")
        )])
    } else {
        Ok(vec![format!(
            "成功{action} {} 个文件到 {target_subdir}/",
            transferred.len()
        )])
    }
}

fn move_zip_files_impl(
    zip_paths: Vec<String>,
    target_subdir: String,
) -> Result<Vec<String>, String> {
    transfer_zip_files_impl(zip_paths, target_subdir, ZipTransferMode::Move)
}

fn copy_zip_files_impl(
    zip_paths: Vec<String>,
    target_subdir: String,
) -> Result<Vec<String>, String> {
    transfer_zip_files_impl(zip_paths, target_subdir, ZipTransferMode::Copy)
}

#[tauri::command]
async fn move_zip_files(
    zip_paths: Vec<String>,
    target_subdir: String,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        move_zip_files_impl(zip_paths, target_subdir)
    })
    .await
    .map_err(|error| format!("task_join_failed: {error}"))?
}

#[tauri::command]
async fn copy_zip_files(
    zip_paths: Vec<String>,
    target_subdir: String,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        copy_zip_files_impl(zip_paths, target_subdir)
    })
    .await
    .map_err(|error| format!("task_join_failed: {error}"))?
}
```

在 `tauri::generate_handler!` 中把 `copy_zip_files` 放在 `move_zip_files` 后面：

```rust
export_app_result,
move_zip_files,
copy_zip_files,
resolve_douyin_unique_id,
```

- [ ] **Step 4: 运行专项 Rust 测试并确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml zip_ -- --nocapture`

Expected: 所有名称包含 `zip_` 的测试 PASS，0 FAIL。

- [ ] **Step 5: 运行完整 Rust 测试作为阶段检查点**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: 所有测试 PASS，0 FAIL。

---

### Task 3: React 两个平台分配按钮

**Files:**
- Modify: `src/App.tsx:1-20`
- Modify: `src/App.tsx:411-610`
- Modify: `tests/workbench-layout.test.ts`

**Interfaces:**
- Consumes: `buildOnlineAllocationPlan(batchRows, platform)`；Tauri 命令 `move_zip_files` 和 `copy_zip_files`。
- Produces: `分配抖音在线包`、`分配头条在线包` 两个独立按钮及真实的组合结果状态。

- [ ] **Step 1: 写失败的工作台源码契约测试**

在 `tests/workbench-layout.test.ts` 加入：

```ts
test("separates Douyin and Toutiao online package allocation", () => {
  assert.match(appSource, /分配抖音在线包/);
  assert.match(appSource, /分配头条在线包/);
  assert.match(appSource, /buildOnlineAllocationPlan\(batchRows, platform\)/);
  assert.match(appSource, /invoke<string\[\]>\("copy_zip_files"/);
  assert.doesNotMatch(appSource, /\? "移动中\.\.\." : "分配在线包"/);
});
```

- [ ] **Step 2: 运行测试并确认缺少两个按钮而失败**

Run: `node --test tests/workbench-layout.test.ts`

Expected: FAIL，首个失败断言指出源码不含 `分配抖音在线包`。

- [ ] **Step 3: 接入分配计划和独立运行状态**

在 `src/App.tsx` 顶部加入：

```ts
import { buildOnlineAllocationPlan } from "./package-allocation";
```

把 `movingOnline` 替换为：

```ts
const [movingDouyinOnline, setMovingDouyinOnline] = useState(false);
const [movingToutiaoOnline, setMovingToutiaoOnline] = useState(false);
```

在 `DetectorWorkbench` 返回 JSX 前加入：

```ts
const douyinOnlinePlan = buildOnlineAllocationPlan(batchRows, "douyin");
const toutiaoOnlinePlan = buildOnlineAllocationPlan(batchRows, "toutiao");

async function handleAllocateOnlinePackages(platform: DetectionPlatform) {
  if (!runtimeReady) {
    onSetStatus("当前是浏览器预览，分配 ZIP 需要在 Tauri 桌面应用中运行");
    return;
  }

  const plan = buildOnlineAllocationPlan(batchRows, platform);
  if (!plan.movePaths.length && !plan.copyPaths.length) return;
  const setMoving = platform === "douyin" ? setMovingDouyinOnline : setMovingToutiaoOnline;
  const summaries: string[] = [];
  const errors: string[] = [];
  setMoving(true);
  try {
    if (plan.movePaths.length) {
      try {
        const result = await invoke<string[]>("move_zip_files", {
          zipPaths: plan.movePaths,
          targetSubdir: plan.targetSubdir,
        });
        summaries.push(result[0] ?? `移动 ${plan.movePaths.length} 个 ZIP 完成`);
      } catch (error) {
        errors.push(`移动失败：${String(error)}`);
      }
    }
    if (plan.copyPaths.length) {
      try {
        const result = await invoke<string[]>("copy_zip_files", {
          zipPaths: plan.copyPaths,
          targetSubdir: plan.targetSubdir,
        });
        summaries.push(result[0] ?? `复制 ${plan.copyPaths.length} 个 ZIP 完成`);
      } catch (error) {
        errors.push(`复制失败：${String(error)}`);
      }
    }

    const platformLabel = platform === "douyin" ? "抖音" : "头条";
    onSetStatus([
      `${platformLabel}在线包分配`,
      ...summaries,
      ...errors,
    ].join("；"));
  } finally {
    setMoving(false);
  }
}
```

- [ ] **Step 4: 用两个按钮替换原统一在线按钮**

用以下 JSX 替换原“分配在线包”按钮：

```tsx
<button
  onClick={() => void handleAllocateOnlinePackages("douyin")}
  disabled={
    !runtimeReady
    || batchRunning
    || douyinOnlinePlan.movePaths.length + douyinOnlinePlan.copyPaths.length === 0
    || movingDouyinOnline
  }
  className="secondary-button"
>
  {movingDouyinOnline ? "移动/复制中..." : "分配抖音在线包"}
</button>
<button
  onClick={() => void handleAllocateOnlinePackages("toutiao")}
  disabled={
    !runtimeReady
    || batchRunning
    || toutiaoOnlinePlan.movePaths.length + toutiaoOnlinePlan.copyPaths.length === 0
    || movingToutiaoOnline
  }
  className="secondary-button"
>
  {movingToutiaoOnline ? "移动/复制中..." : "分配头条在线包"}
</button>
```

删除不再使用的 `movingOnline`、`setMovingOnline` 和原统一在线按钮事件处理器。其他三个分配按钮不改。

- [ ] **Step 5: 运行工作台与分配器测试并确认通过**

Run: `node --test tests/workbench-layout.test.ts tests/package-allocation.test.ts`

Expected: 所有测试 PASS，0 FAIL。

- [ ] **Step 6: 运行前端构建作为类型检查点**

Run: `npm run build`

Expected: TypeScript 和 Vite 构建成功，退出码 0。

---

### Task 4: 全量验证与 macOS 应用产物

**Files:**
- Verify: `src/package-allocation.ts`
- Verify: `tests/package-allocation.test.ts`
- Verify: `src/App.tsx`
- Verify: `tests/workbench-layout.test.ts`
- Verify: `src-tauri/src/lib.rs`
- Verify artifact: `src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app`

**Interfaces:**
- Consumes: Tasks 1-3 的完整实现。
- Produces: 前端、Rust 和打包应用的最新验证证据。

- [ ] **Step 1: 运行完整前端测试**

Run: `npm test`

Expected: 所有测试 PASS，0 FAIL。

- [ ] **Step 2: 运行前端生产构建**

Run: `npm run build`

Expected: TypeScript 和 Vite 构建成功，退出码 0。

- [ ] **Step 3: 运行完整 Rust 测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: 所有测试 PASS，0 FAIL。

- [ ] **Step 4: 构建 macOS `.app`**

Run: `npm run tauri build -- --bundles app`

Expected: 退出码 0，并生成 `src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app`。

- [ ] **Step 5: 核对产物和源代码契约**

Run: `test -d "src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app" && rg -n "分配抖音在线包|分配头条在线包|copy_zip_files|douyin_online|toutiao_online" src/App.tsx src/package-allocation.ts src-tauri/src/lib.rs`

Expected: 命令退出码 0；两个按钮、复制命令和两个目录均命中。

- [ ] **Step 6: 记录未完成的真实桌面交互边界**

若当前环境不能安全打开 GUI 或没有可分配的真实 ZIP，最终报告明确区分：自动测试与打包已验证；实际按钮点击和真实目录落盘仍需在打包应用中使用测试 ZIP 复核。不得把仅有构建成功描述成真实文件分配已经执行。
