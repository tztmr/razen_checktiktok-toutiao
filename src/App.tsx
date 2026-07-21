import React, { memo, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import {
  buildBatchDetectionOptions,
  queueBatchOptionFromEvent,
  type BatchDetectionOptions,
  type DetectionPlatform,
  type DouyinDetectionOptions,
  type ToutiaoDetectionOptions,
} from "./batch-options";
import {
  formatDetectionDuration,
  formatToutiaoTokenStatus,
  getDetectionStatusLabel,
  resolveDetectionStatus,
} from "./workbench-ui";
import { buildOnlineAllocationPlan } from "./package-allocation";
import { useModalDialog } from "./use-modal-dialog";
import "./App.css";

type AppSummary = {
  sourceZip: string;
  appId: string;
  displayName: string;
  subtitle: string;
  appKind: string;
  logoText: string;
  logoColor: string;
  totalFiles: number;
  candidateFiles: number;
};

type ZipScanSummary = {
  sourcePath: string;
  sourceMode: string;
  sourceZips: string[];
  zipCount: number;
  batchRoot: string | null;
  appCount: number;
  fileCount: number;
  cacheHit: boolean;
  apps: AppSummary[];
};

type CandidateFile = {
  sourceZip: string;
  appId: string;
  innerPath: string;
  fileType: string;
  parameterScope: string;
  size: number;
  parseSupported: boolean;
};

type ParseResult = {
  sourceZip: string;
  appId: string;
  innerPath: string;
  fileType: string;
  parseStatus: string;
  parsedData: unknown;
  meta: Record<string, unknown>;
  error?: string | null;
};

type DouyinUniqueIdResult = {
  uid: string;
  secUid: string;
  uniqueId: string;
  status: string;
  error?: string | null;
};

type ToutiaoSecuidResult = {
  ttUid: string;
  ttSecuid: string;
  status: string;
  error?: string | null;
};

type DouyinRequestParamsResult = {
  sourceZip: string;
  sourcePlistPath: string;
  sourceCookiePath?: string | null;
  secUserId: string;
  cookieHeader: string;
  headerCount: number;
  headerText: string;
  headers: Record<string, string>;
  status?: string;
  error?: string | null;
};

type DouyinSessionBindings = {
  summary: string;
  toutiao: string;
  toutiaoPlatformScreenName: string;
  qq: string;
  qqPlatformScreenName: string;
  google: string;
  googlePlatformScreenName: string;
  appleId: string;
  appleIdPlatformScreenName: string;
  wechat: string;
  wechatPlatformScreenName: string;
};

type DouyinPasswordStatusResult = {
  sourceZip: string;
  sourceCookiePath?: string | null;
  sessionId: string;
  hasPassword?: boolean | null;
  accountName?: string | null;
  registerTime?: string | null;
  bindings: DouyinSessionBindings;
  status: string;
  error?: string | null;
};

type DouyinCertificationStatusResult = {
  sourceZip: string;
  sourcePlistPath?: string | null;
  isVerified?: boolean | null;
  accountName?: string | null;
  status: string;
  error?: string | null;
};

type DouyinTokenEndpointResult = {
  name: string;
  url: string;
  httpStatus?: number | null;
  statusCode?: number | null;
  status: string;
  message?: string | null;
  uid?: string | null;
  secUid?: string | null;
  nickname?: string | null;
  phoneNumber?: string | null;
  registerTime?: string | null;
  awemeCount?: string | null;
  followingCount?: string | null;
  likedCount?: string | null;
  functions: Array<{funcName: string; funcAvailable: boolean}>;
};

type DouyinTokenStatusResult = {
  sourceZip: string;
  sourcePlistPath?: string | null;
  sourceCookiePath?: string | null;
  tokenPreview: string;
  odinTtPreview: string;
  localPhoneNumber?: string | null;
  status: string;
  validEndpointCount: number;
  endpoints: DouyinTokenEndpointResult[];
  functions: Array<{funcName: string; funcAvailable: boolean}>;
  error?: string | null;
};

type DouyinAccountCredentialItem = {
  uid: string;
  nickname: string;
  secUid: string;
  uniqueId: string;
  shortId: string;
  sessionId: string;
  sessionIdPreview: string;
  accessToken: string;
  accessTokenPreview: string;
  openId: string;
  openIdPreview: string;
  authTimeLabel: string;
  isCurrent: boolean;
  phoneNumber: string;
  registerTime: string;
  awemeCount: string;
  followingCount: string;
  likedCount: string;
  bindings: DouyinSessionBindings;
  hasPassword?: boolean | null;
  isVerified?: boolean | null;
  normalFunctions: string[];
};

type DouyinAccountCredentialResult = {
  sourceZip: string;
  sourcePlistPath?: string | null;
  sourceCookiePath?: string | null;
  currentSessionIdPreview: string;
  currentTokenPreview: string;
  currentOdinTtPreview: string;
  accountCount: number;
  accounts: DouyinAccountCredentialItem[];
  status: string;
  error?: string | null;
};

type BatchDetectionRow = {
  key: string;
  sourceZip: string;
  appId: string;
  appName: string;
  appType: "douyin" | "toutiao";
  aid: string;
  orderLabel: string;
  fullParams: string;
  childLockStatus: string;
  accountName: string;
  bindingSummary: string;
  toutiaoBinding: string;
  toutiaoPlatformScreenName: string;
  qqBinding: string;
  qqPlatformScreenName: string;
  googleBinding: string;
  googlePlatformScreenName: string;
  appleIdBinding: string;
  appleIdPlatformScreenName: string;
  wechatBinding: string;
  wechatPlatformScreenName: string;
  secUid: string;
  uid: string;
  uniqueId: string;
  phoneNumber: string;
  registerTime: string;
  awemeCount: string;
  followingCount: string;
  likedCount: string;
  tokenStatus: string;
  passwordStatus: string;
  certificationStatus: string;
  normalFunctions: string;
  limitedFunctions: string;
  durationMs: number | null;
  status: "pending" | "checking" | "online" | "offline" | "failed" | "skipped";
  error?: string | null;
};

type DetailTab = "overview" | "files" | "result" | "raw";

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

type ScanProgressPayload = {
  stage: string;
  message: string;
  current: number;
  total: number;
  currentZip?: string | null;
  percent: number;
};

type PreferenceEntry = {
  path: string;
  value: string;
  valueType: string;
};

type SqliteTable = {
  name: string;
  columns: Array<{ name: string; dataType: string }>;
  rows: Array<Record<string, unknown>>;
};

type SqliteParsedData = {
  tables: SqliteTable[];
};

type CookiePreviewItem = {
  name: string;
  domain: string;
  path: string;
  value: string;
  expiresLabel: string;
  createdLabel?: string;
  flagsLabel?: string;
};

const FULL_PARAMS_PREVIEW_LENGTH = 140;

const EMPTY_DOUYIN_SESSION_BINDINGS: DouyinSessionBindings = {
  summary: "",
  toutiao: "",
  toutiaoPlatformScreenName: "",
  qq: "",
  qqPlatformScreenName: "",
  google: "",
  googlePlatformScreenName: "",
  appleId: "",
  appleIdPlatformScreenName: "",
  wechat: "",
  wechatPlatformScreenName: "",
};

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const DetectorRow = memo(function DetectorRow({
  row,
  index,
  isSelected,
  onSelect,
}: {
  row: BatchDetectionRow;
  index: number;
  isSelected?: boolean;
  onSelect?: (row: BatchDetectionRow, opener: HTMLElement) => void;
}) {
  const clickable = typeof onSelect === "function";
  const sessionValue = extractSessionFromFullParams(row.fullParams);

  return (
    <tr
      className={`detector-row detector-row-${row.status} ${isSelected ? "detector-row-selected" : ""} ${clickable ? "detector-row-clickable" : ""}`}
      onClick={clickable ? (event) => onSelect(row, event.currentTarget) : undefined}
      onKeyDown={
        clickable
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onSelect(row, event.currentTarget);
              }
            }
          : undefined
      }
      tabIndex={clickable ? 0 : undefined}
      role={clickable ? "button" : undefined}
      aria-label={clickable ? `查看 ${row.appName} 的检测详情` : undefined}
    >
      <td>{index + 1}</td>
      <td className="detector-status-cell">
        <span className={`detector-status detector-status-${row.status}`}>
          {getDetectionStatusLabel(row.status)}
        </span>
      </td>
      <td>{row.appName}</td>
      <td>{row.accountName}</td>
      <td className="detector-full-params-cell">
        <code title={row.fullParams}>
          {formatTextPreview(row.fullParams, FULL_PARAMS_PREVIEW_LENGTH)}
        </code>
      </td>
      <td className="detector-session-cell">
        <code title={sessionValue}>{formatTextPreview(sessionValue, 42)}</code>
      </td>
      <td>{row.childLockStatus}</td>
      <td>{row.certificationStatus}</td>
      <td>{row.aid}</td>
      <td>{row.bindingSummary}</td>
      <td>{row.toutiaoBinding}</td>
      <td>{row.toutiaoPlatformScreenName}</td>
      <td>{row.qqBinding}</td>
      <td>{row.qqPlatformScreenName}</td>
      <td>{row.googleBinding}</td>
      <td>{row.googlePlatformScreenName}</td>
      <td>{row.appleIdBinding}</td>
      <td>{row.appleIdPlatformScreenName}</td>
      <td>{row.wechatBinding}</td>
      <td>{row.wechatPlatformScreenName}</td>
      <td>{row.secUid}</td>
      <td>{row.uid}</td>
      <td>{row.uniqueId}</td>
      <td>{row.phoneNumber}</td>
      <td>{row.registerTime}</td>
      <td>{row.awemeCount}</td>
      <td>{row.followingCount}</td>
      <td>{row.likedCount}</td>
      <td>{row.tokenStatus}</td>
      <td>{row.passwordStatus}</td>
      <td>{formatDetectionDuration(row.durationMs)}</td>
      <td>{row.limitedFunctions || "-"}</td>
      <td>{row.normalFunctions || "-"}</td>
    </tr>
  );
});

type DouyinOptionsDispatch = React.Dispatch<React.SetStateAction<DouyinDetectionOptions>>;
type ToutiaoOptionsDispatch = React.Dispatch<React.SetStateAction<ToutiaoDetectionOptions>>;

const DetectorWorkbench = memo(function DetectorWorkbench({
  runtimeReady,
  batchRows,
  batchStats,
  douyinOptions,
  toutiaoOptions,
  batchRunning,
  batchStartedAt,
  batchElapsedMs,
  trackedAppCount,
  scanSummary,
  status,
  loading,
  selectedRowKey,
  onOpenScanModal,
  onSelectRow,
  onRunDetection,
  onStopDetection,
  onClearRows,
  onExportRows,
  onDouyinOptionsChange,
  onToutiaoOptionsChange,
  onSetStatus,
}: {
  runtimeReady: boolean;
  batchRows: BatchDetectionRow[];
  batchStats: { total: number; online: number; offline: number; normalFunc: number; limitedFunc: number };
  douyinOptions: DouyinDetectionOptions;
  toutiaoOptions: ToutiaoDetectionOptions;
  batchRunning: boolean;
  batchStartedAt: number | null;
  batchElapsedMs: number;
  trackedAppCount: number;
  scanSummary: ZipScanSummary | null;
  status: string;
  loading: boolean;
  selectedRowKey: string;
  onOpenScanModal: (opener: HTMLElement) => void;
  onSelectRow: (row: BatchDetectionRow, opener: HTMLElement) => void;
  onRunDetection: (platform: DetectionPlatform) => void;
  onStopDetection: () => void;
  onClearRows: () => void;
  onExportRows: (filter: "all" | "online" | "offline") => void;
  onDouyinOptionsChange: DouyinOptionsDispatch;
  onToutiaoOptionsChange: ToutiaoOptionsDispatch;
  onSetStatus: (msg: string) => void;
}) {
  const elapsedRef = useRef<HTMLSpanElement | null>(null);
  const [movingDouyinOnline, setMovingDouyinOnline] = useState(false);
  const [movingToutiaoOnline, setMovingToutiaoOnline] = useState(false);
  const [movingOffline, setMovingOffline] = useState(false);
  const [movingNormalFunc, setMovingNormalFunc] = useState(false);
  const [movingLimitedFunc, setMovingLimitedFunc] = useState(false);
  const douyinAppCount = scanSummary?.apps.filter((app) => getTrackedAppType(app.appId) === "douyin").length ?? 0;
  const toutiaoAppCount = scanSummary?.apps.filter((app) => getTrackedAppType(app.appId) === "toutiao").length ?? 0;
  const surfaceState = !runtimeReady ? "预览" : loading ? "处理中" : batchRunning ? "检测中" : scanSummary ? "就绪" : "待扫描";
  const sourceSummary = !runtimeReady
    ? "当前是浏览器预览，扫描与检测仅在 Tauri 桌面应用内可用"
    : scanSummary
    ? `${formatScanMode(scanSummary.sourceMode)} · ZIP ${scanSummary.zipCount} · APP ${trackedAppCount}`
    : "点击“扫描资源”开始导入 ZIP 或目录";
  const emptyMessage = scanSummary
    ? trackedAppCount
      ? "批量检测表已就绪，点击“开始检测”或直接点行查看详情。"
      : "扫描完成，但暂时没有命中抖音或今日头条目标 APP。"
    : runtimeReady
      ? "还没有扫描任何 ZIP 或目录，先点击右上角“扫描资源”打开弹窗。"
      : "当前是浏览器预览，第三版布局可直接查看，扫描与检测请在 Tauri 桌面应用中完成。";

  useEffect(() => {
    if (!batchRunning || !batchStartedAt) return;
    const startedAt = batchStartedAt;
    let rafId: number;
    function tick() {
      if (elapsedRef.current) {
        elapsedRef.current.textContent = String(Math.round(Date.now() - startedAt));
      }
      rafId = requestAnimationFrame(tick);
    }
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [batchRunning, batchStartedAt]);

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

  return (
    <section className="detector-workbench">
      <aside className="detector-actions">
        <div className="detector-rail-title">批量操作</div>
        <section className="detector-action-group">
          <span className="detector-action-group-title">列表</span>
          <button className="danger-button" onClick={onClearRows} disabled={batchRunning || !batchRows.length}>清空列表</button>
        </section>
        <section className="detector-action-group">
          <span className="detector-action-group-title">数据导出</span>
          <button onClick={() => onExportRows("all")} disabled={!batchRows.length}>导出全部数据</button>
          <button onClick={() => onExportRows("offline")} disabled={!batchRows.length}>导出掉线数据</button>
          <button onClick={() => onExportRows("online")} disabled={!batchRows.length}>导出在线数据</button>
        </section>
        <section className="detector-action-group">
          <span className="detector-action-group-title">包分配</span>
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
        <button
          onClick={async () => {
            if (!runtimeReady) {
              onSetStatus("当前是浏览器预览，分配 ZIP 需要在 Tauri 桌面应用中运行");
              return;
            }
            const paths = batchRows
              .filter((r) => r.status === "offline" || r.status === "failed")
              .map((r) => r.sourceZip)
              .filter(uniqueZipPaths);
            if (!paths.length) return;
            setMovingOffline(true);
               try {
                 const result = await invoke<string[]>("move_zip_files", { zipPaths: paths, targetSubdir: "offline" });
                 onSetStatus(result[0] ?? "完成");
               } catch (e) {
                 onSetStatus(String(e));
            } finally {
              setMovingOffline(false);
            }
          }}
          disabled={!runtimeReady || batchRunning || batchStats.offline === 0 || movingOffline}
          className="secondary-button"
        >
          {movingOffline ? "移动中..." : "分配掉线包"}
        </button>
        <button
          onClick={async () => {
            if (!runtimeReady) {
              onSetStatus("当前是浏览器预览，分配 ZIP 需要在 Tauri 桌面应用中运行");
              return;
            }
            const paths = batchRows
              .filter((r) => r.normalFunctions && r.normalFunctions.trim().length > 0)
              .map((r) => r.sourceZip)
              .filter(uniqueZipPaths);
            if (!paths.length) return;
            setMovingNormalFunc(true);
               try {
                 const result = await invoke<string[]>("move_zip_files", { zipPaths: paths, targetSubdir: "normal_functions" });
                 onSetStatus(result[0] ?? "完成");
               } catch (e) {
                 onSetStatus(String(e));
            } finally {
              setMovingNormalFunc(false);
            }
          }}
          disabled={!runtimeReady || batchRunning || batchStats.normalFunc === 0 || movingNormalFunc}
          className="secondary-button"
        >
          {movingNormalFunc ? "移动中..." : "分配正常功能包"}
        </button>
        <button
          onClick={async () => {
            if (!runtimeReady) {
              onSetStatus("当前是浏览器预览，分配 ZIP 需要在 Tauri 桌面应用中运行");
              return;
            }
            const paths = batchRows
              .filter((r) => r.limitedFunctions && r.limitedFunctions.trim().length > 0)
              .map((r) => r.sourceZip)
              .filter(uniqueZipPaths);
            if (!paths.length) return;
            setMovingLimitedFunc(true);
               try {
                 const result = await invoke<string[]>("move_zip_files", { zipPaths: paths, targetSubdir: "limited_functions" });
                 onSetStatus(result[0] ?? "完成");
               } catch (e) {
                 onSetStatus(String(e));
            } finally {
              setMovingLimitedFunc(false);
            }
          }}
          disabled={!runtimeReady || batchRunning || batchStats.limitedFunc === 0 || movingLimitedFunc}
          className="secondary-button"
        >
          {movingLimitedFunc ? "移动中..." : "分配限制功能包"}
        </button>
        </section>
      </aside>

      <article className="detector-table-panel">
        <div className="detector-toolbar">
          <div className="detector-toolbar-copy">
            <span className={`detector-surface-badge detector-surface-badge-${loading ? "loading" : batchRunning ? "running" : scanSummary ? "ready" : "idle"}`}>
              {surfaceState}
            </span>
            <div className="detector-toolbar-copy-text">
              <strong>{status}</strong>
              <small>{sourceSummary}</small>
            </div>
          </div>

          <div className="detector-stats">
            <span>累计检测 {batchStats.total}</span>
            <span>在线 {batchStats.online}</span>
            <span>掉线 {batchStats.offline}</span>
            <span ref={elapsedRef}>用时 {Math.round(batchElapsedMs)} ms</span>
          </div>

          <div className="detector-toolbar-actions">
            <button className="secondary-button detector-toolbar-button" onClick={(event) => onOpenScanModal(event.currentTarget)} disabled={loading}>
              扫描资源
            </button>
          </div>
        </div>
        <div className="detector-table-stage">
          <div className="detector-table-scroll">
            <table className="detector-table">
              <thead>
                <tr>
                  <th>序号</th>
                  <th>状态</th>
                  <th>APP</th>
                  <th>账号</th>
                  <th>全参</th>
                  <th>session</th>
                  <th>儿童锁</th>
                  <th>实名状态</th>
                  <th>aid</th>
                  <th>绑定</th>
                  <th>头条</th>
                  <th>头条platform_screen_name</th>
                  <th>QQ</th>
                  <th>QQplatform_screen_name</th>
                  <th>谷歌</th>
                  <th>谷歌platform_screen_name</th>
                  <th>ID</th>
                  <th>IDplatform_screen_name</th>
                  <th>微信</th>
                  <th>微信platform_screen_name</th>
                  <th>sec_uid</th>
                  <th>uid</th>
                  <th>unique_id</th>
                  <th>手机号</th>
                  <th>注册时间</th>
                  <th>作品数</th>
                  <th>关注数</th>
                  <th>点赞数</th>
                  <th>Token</th>
                  <th>密码状态</th>
                  <th>用时</th>
                  <th>限制功能</th>
                  <th>正常功能</th>
                </tr>
              </thead>
              <tbody>
                {batchRows.map((row, index) => (
                  <DetectorRow
                    key={row.key}
                    row={row}
                    index={index}
                    isSelected={selectedRowKey === row.key}
                    onSelect={onSelectRow}
                  />
                ))}
              </tbody>
            </table>
          </div>
          {!batchRows.length ? (
            <div className="detector-empty-overlay">
              <div className="detector-empty-state">
                <strong>还没有检测数据</strong>
                <span>{emptyMessage}</span>
                <button className="secondary-button" onClick={(event) => onOpenScanModal(event.currentTarget)} disabled={loading}>
                  扫描资源
                </button>
              </div>
            </div>
          ) : null}
        </div>
      </article>

      <aside className="detector-options">
        <div className="detector-rail-title">检测配置</div>
        <section className="platform-detection-card platform-detection-card-douyin">
          <header className="platform-detection-header">
            <div>
              <span className="platform-mark" aria-hidden="true">抖</span>
              <strong>抖音检测</strong>
            </div>
            <small>{douyinAppCount} 个 APP</small>
          </header>
          <div className="platform-detection-controls">
            <label>
              <input type="checkbox" checked={douyinOptions.token} disabled={batchRunning} onChange={(event) => queueBatchOptionFromEvent(onDouyinOptionsChange, "token", event, (target) => target.checked)} />
              Token
            </label>
            <label>
              <input type="checkbox" checked={douyinOptions.password} disabled={batchRunning} onChange={(event) => queueBatchOptionFromEvent(onDouyinOptionsChange, "password", event, (target) => target.checked)} />
              密码
            </label>
            <label>
              <input type="checkbox" checked={douyinOptions.certification} disabled={batchRunning} onChange={(event) => queueBatchOptionFromEvent(onDouyinOptionsChange, "certification", event, (target) => target.checked)} />
              实名
            </label>
            <label>
              <input type="checkbox" checked={douyinOptions.aid} disabled={batchRunning} onChange={(event) => queueBatchOptionFromEvent(onDouyinOptionsChange, "aid", event, (target) => target.checked)} />
              aid
            </label>
            <label>
              <input type="checkbox" checked={douyinOptions.registrationTime} disabled={batchRunning} onChange={(event) => queueBatchOptionFromEvent(onDouyinOptionsChange, "registrationTime", event, (target) => target.checked)} />
              注册时间
            </label>
          </div>
          <button
            className="detector-primary-button platform-start-button"
            onClick={() => onRunDetection("douyin")}
            disabled={!runtimeReady || douyinAppCount === 0 || batchRunning}
          >
            开始检测抖音
          </button>
        </section>

        <section className="platform-detection-card platform-detection-card-toutiao">
          <header className="platform-detection-header">
            <div>
              <span className="platform-mark" aria-hidden="true">头</span>
              <strong>今日头条检测</strong>
            </div>
            <small>{toutiaoAppCount} 个 APP</small>
          </header>
          <div className="platform-detection-controls">
            <label>
              <input type="checkbox" checked={toutiaoOptions.token} disabled={batchRunning} onChange={(event) => queueBatchOptionFromEvent(onToutiaoOptionsChange, "token", event, (target) => target.checked)} />
              Token
            </label>
            <label>
              <input type="checkbox" checked={toutiaoOptions.certification} disabled={batchRunning} onChange={(event) => queueBatchOptionFromEvent(onToutiaoOptionsChange, "certification", event, (target) => target.checked)} />
              登录/实名状态
            </label>
          </div>
          <p className="platform-detection-hint">检测 Token 在线状态、账号资料及实名状态</p>
          <button
            className="detector-primary-button platform-start-button"
            onClick={() => onRunDetection("toutiao")}
            disabled={!runtimeReady || toutiaoAppCount === 0 || batchRunning}
          >
            开始检测头条
          </button>
        </section>

        <button className="secondary-button detector-stop-button" onClick={onStopDetection} disabled={!runtimeReady || !batchRunning}>
          停止当前检测
        </button>
      </aside>
    </section>
  );
});

function App() {
  const tauriReady = hasTauriRuntime();
  const [sourcePath, setSourcePath] = useState("");
  const [scanSummary, setScanSummary] = useState<ZipScanSummary | null>(null);
  const [selectedAppKey, setSelectedAppKey] = useState("");
  const [files, setFiles] = useState<CandidateFile[]>([]);
  const [selectedFile, setSelectedFile] = useState<CandidateFile | null>(null);
  const [parseResult, setParseResult] = useState<ParseResult | null>(null);
  const [selectedTableName, setSelectedTableName] = useState("");
  const [sqliteTableFilter, setSqliteTableFilter] = useState("");
  const [sqliteRowSearch, setSqliteRowSearch] = useState("");
  const [status, setStatus] = useState("等待扫描路径");
  const [loading, setLoading] = useState(false);
  const [douyinUniqueId, setDouyinUniqueId] = useState<DouyinUniqueIdResult | null>(null);
  const [douyinRequestParams, setDouyinRequestParams] = useState<DouyinRequestParamsResult | null>(null);
  const [toutiaoSecuid, setToutiaoSecuid] = useState<ToutiaoSecuidResult | null>(null);
  const [douyinPasswordStatus, setDouyinPasswordStatus] = useState<DouyinPasswordStatusResult | null>(null);
  const [douyinCertificationStatus, setDouyinCertificationStatus] = useState<DouyinCertificationStatusResult | null>(null);
  const [douyinTokenStatus, setDouyinTokenStatus] = useState<DouyinTokenStatusResult | null>(null);
  const [douyinAccountCredentials, setDouyinAccountCredentials] = useState<DouyinAccountCredentialResult | null>(null);
  const [toutiaoCertificationStatus, setToutiaoCertificationStatus] = useState<ToutiaoCertificationStatusResult | null>(null);
  const [scanProgress, setScanProgress] = useState<ScanProgressPayload | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  const [batchRows, setBatchRows] = useState<BatchDetectionRow[]>([]);
  const [douyinOptions, setDouyinOptions] = useState<DouyinDetectionOptions>({
    token: true,
    password: true,
    certification: true,
    aid: true,
    registrationTime: true,
  });
  const [toutiaoOptions, setToutiaoOptions] = useState<ToutiaoDetectionOptions>({
    token: true,
    certification: true,
  });
  const [batchRunning, setBatchRunning] = useState(false);
  const [batchStartedAt, setBatchStartedAt] = useState<number | null>(null);
  const [batchElapsedMs, setBatchElapsedMs] = useState(0);
  const [isScanModalOpen, setIsScanModalOpen] = useState(false);
  const [isDetailModalOpen, setIsDetailModalOpen] = useState(false);
  const [activeDetailTab, setActiveDetailTab] = useState<DetailTab>("overview");
  const [selectedBatchRowKey, setSelectedBatchRowKey] = useState("");
  const batchStopRef = useRef(false);
  const batchRowsRef = useRef<BatchDetectionRow[]>([]);
  const batchFlushPending = useRef(false);
  const scanDialog = useModalDialog({
    open: isScanModalOpen,
    canClose: !loading,
    onRequestClose: handleCloseScanModal,
  });
  const detailDialog = useModalDialog({
    open: isDetailModalOpen,
    canClose: true,
    onRequestClose: handleCloseDetailModal,
  });

  function flushBatchRows() {
    batchFlushPending.current = false;
    setBatchRows([...batchRowsRef.current]);
  }

  function scheduleBatchFlush() {
    if (!batchFlushPending.current) {
      batchFlushPending.current = true;
      requestAnimationFrame(() => flushBatchRows());
    }
  }

  function replaceBatchRowWithMultiple(key: string, nextRows: BatchDetectionRow[]) {
    batchRowsRef.current = batchRowsRef.current.flatMap((row) =>
      row.key === key ? nextRows : [row],
    );
    scheduleBatchFlush();
  }

  function patchBatchRow(key: string, patch: Partial<BatchDetectionRow>) {
    batchRowsRef.current = batchRowsRef.current.map((row) =>
      row.key === key ? { ...row, ...patch } : row,
    );
    scheduleBatchFlush();
  }

  const trackedApps = useMemo(
    () => (scanSummary?.apps ?? []).filter((app) => getTrackedAppType(app.appId) !== null),
    [scanSummary],
  );
  const selectedApp = useMemo(
    () => trackedApps.find((app) => buildAppKey(app) === selectedAppKey) ?? null,
    [trackedApps, selectedAppKey],
  );
  const selectedAppOrderLabel = useMemo(() => {
    const appIndex = trackedApps.findIndex((app) => buildAppKey(app) === selectedAppKey);
    return appIndex >= 0 ? String(appIndex + 1).padStart(3, "0") : "";
  }, [trackedApps, selectedAppKey]);
  const selectedAppType = useMemo(
    () => getTrackedAppType(selectedApp?.appId ?? selectedFile?.appId ?? ""),
    [selectedApp?.appId, selectedFile?.appId],
  );
  const selectedBatchRow = useMemo(
    () => batchRows.find((row) => row.key === selectedBatchRowKey) ?? null,
    [batchRows, selectedBatchRowKey],
  );
  const selectedBatchApp = useMemo(
    () => trackedApps.find((app) => buildAppKey(app) === selectedBatchRowKey) ?? null,
    [trackedApps, selectedBatchRowKey],
  );
  const sqliteTables = useMemo(() => {
    if (parseResult?.fileType !== "sqlite") return [];
    const parsedData = parseResult.parsedData as SqliteParsedData | null;
    return Array.isArray(parsedData?.tables) ? parsedData.tables : [];
  }, [parseResult]);
  const filteredSqliteTables = useMemo(() => {
    const keyword = sqliteTableFilter.trim().toLowerCase();
    if (!keyword) return sqliteTables;
    return sqliteTables.filter((table) => table.name.toLowerCase().includes(keyword));
  }, [sqliteTableFilter, sqliteTables]);
  const selectedSqliteTable = useMemo(
    () => filteredSqliteTables.find((table) => table.name === selectedTableName) ?? filteredSqliteTables[0] ?? null,
    [filteredSqliteTables, selectedTableName],
  );
  const filteredSqliteRows = useMemo(() => {
    if (!selectedSqliteTable) return [];
    const keyword = sqliteRowSearch.trim().toLowerCase();
    if (!keyword) return selectedSqliteTable.rows;
    return selectedSqliteTable.rows.filter((row) => JSON.stringify(row).toLowerCase().includes(keyword));
  }, [selectedSqliteTable, sqliteRowSearch]);
  const cookiePreviewItems = useMemo(() => {
    if (selectedFile?.parameterScope !== "cookies") return [];
    if (parseResult?.fileType === "binarycookies") return getBinaryCookieItems(parseResult.parsedData);
    return filteredSqliteRows
      .map<CookiePreviewItem | null>((row) => normalizeCookieRow(row))
      .filter((row): row is CookiePreviewItem => Boolean(row));
  }, [filteredSqliteRows, parseResult, selectedFile]);
  const preferenceEntries = useMemo(() => {
    if (selectedFile?.parameterScope !== "preferences" || !parseResult) return [];
    return flattenPreferenceEntries(parseResult.parsedData);
  }, [parseResult, selectedFile]);
  const visibleFiles = useMemo(() => {
    if (!selectedAppType) return files;
    return files
      .filter((file) => file.parameterScope === "preferences" || file.parameterScope === "cookies")
      .filter((file) => matchesTrackedFile(selectedAppType, file));
  }, [files, selectedAppType]);
  const douyinPreferenceSummary = useMemo(
    () =>
      selectedAppType === "douyin" && selectedFile?.parameterScope === "preferences"
        ? extractDouyinPreferenceSummary(parseResult?.parsedData)
        : null,
    [parseResult?.parsedData, selectedAppType, selectedFile?.parameterScope],
  );
  const toutiaoPreferenceSummary = useMemo(
    () =>
      selectedAppType === "toutiao" && selectedFile?.parameterScope === "preferences"
        ? extractToutiaoPreferenceSummary(parseResult?.parsedData)
        : null,
    [parseResult?.parsedData, selectedAppType, selectedFile?.parameterScope],
  );
  const cookieSummary = useMemo(
    () =>
      selectedFile?.parameterScope === "cookies"
        ? buildCookieSummary(parseResult?.parsedData, cookiePreviewItems)
        : null,
    [cookiePreviewItems, parseResult?.parsedData, selectedFile?.parameterScope],
  );
  const batchStats = useMemo(() => {
    const online = batchRows.filter((row) => row.status === "online").length;
    const offline = batchRows.filter((row) => row.status === "offline" || row.status === "failed").length;
    const normalFunc = batchRows.filter((row) => row.normalFunctions && row.normalFunctions.trim().length > 0).length;
    const limitedFunc = batchRows.filter((row) => row.limitedFunctions && row.limitedFunctions.trim().length > 0).length;
    return { total: batchRows.length, online, offline, normalFunc, limitedFunc };
  }, [batchRows]);

  useEffect(() => {
    const rows = buildInitialBatchRows(trackedApps, "all");
    batchRowsRef.current = rows;
    setBatchRows(rows);
    setBatchElapsedMs(0);
    setBatchStartedAt(null);
    batchStopRef.current = false;
  }, [trackedApps]);

  useEffect(() => {
    if (selectedAppType !== "douyin" || selectedFile?.parameterScope !== "preferences" || !douyinPreferenceSummary?.dySecUid) {
      setDouyinUniqueId(null);
      return;
    }
    let cancelled = false;
    setDouyinUniqueId({ uid: "", secUid: douyinPreferenceSummary.dySecUid, uniqueId: "", status: "loading", error: null });
    void invoke<DouyinUniqueIdResult>("resolve_douyin_unique_id", { secUid: douyinPreferenceSummary.dySecUid })
      .then((result) => { if (!cancelled) setDouyinUniqueId(result); })
      .catch((error) => { if (!cancelled) setDouyinUniqueId({ uid: "", secUid: douyinPreferenceSummary.dySecUid, uniqueId: "", status: "error", error: String(error) }); });
    return () => { cancelled = true; };
  }, [douyinPreferenceSummary?.dySecUid, selectedAppType, selectedFile?.parameterScope]);

  useEffect(() => {
    if (selectedAppType !== "douyin" || selectedFile?.parameterScope !== "preferences" || !selectedFile.sourceZip) {
      setDouyinPasswordStatus(null);
      return;
    }
    let cancelled = false;
    setDouyinPasswordStatus({ sourceZip: selectedFile.sourceZip, sourceCookiePath: null, sessionId: "", hasPassword: null, accountName: null, bindings: EMPTY_DOUYIN_SESSION_BINDINGS, status: "loading", error: null });
    void invoke<DouyinPasswordStatusResult>("check_douyin_password_status", { zipPath: selectedFile.sourceZip })
      .then((result) => { if (!cancelled) setDouyinPasswordStatus(result); })
      .catch((error) => { if (!cancelled) setDouyinPasswordStatus({ sourceZip: selectedFile.sourceZip, sourceCookiePath: null, sessionId: "", hasPassword: null, accountName: null, bindings: EMPTY_DOUYIN_SESSION_BINDINGS, status: "error", error: String(error) }); });
    return () => { cancelled = true; };
  }, [selectedAppType, selectedFile?.parameterScope, selectedFile?.sourceZip]);

  useEffect(() => {
    if (selectedAppType !== "douyin" || selectedFile?.parameterScope !== "preferences" || !selectedFile.sourceZip) {
      setDouyinCertificationStatus(null);
      return;
    }
    let cancelled = false;
    setDouyinCertificationStatus({ sourceZip: selectedFile.sourceZip, sourcePlistPath: selectedFile.innerPath, isVerified: null, accountName: null, status: "loading", error: null });
    void invoke<DouyinCertificationStatusResult>("check_douyin_certification_status", { zipPath: selectedFile.sourceZip })
      .then((result) => { if (!cancelled) setDouyinCertificationStatus(result); })
      .catch((error) => { if (!cancelled) setDouyinCertificationStatus({ sourceZip: selectedFile.sourceZip, sourcePlistPath: selectedFile.innerPath, isVerified: null, accountName: null, status: "error", error: String(error) }); });
    return () => { cancelled = true; };
  }, [selectedAppType, selectedFile?.parameterScope, selectedFile?.sourceZip, selectedFile?.innerPath]);

  useEffect(() => {
    if (selectedAppType !== "douyin" || selectedFile?.parameterScope !== "preferences" || !selectedFile.sourceZip) {
      setDouyinTokenStatus(null);
      return;
    }
    let cancelled = false;
    setDouyinTokenStatus({ sourceZip: selectedFile.sourceZip, sourcePlistPath: selectedFile.innerPath, sourceCookiePath: null, tokenPreview: "", odinTtPreview: "", status: "loading", validEndpointCount: 0, endpoints: [], functions: [], error: null });
    void invoke<DouyinTokenStatusResult>("check_douyin_token_status", { zipPath: selectedFile.sourceZip })
      .then((result) => { if (!cancelled) setDouyinTokenStatus(result); })
      .catch((error) => { if (!cancelled) setDouyinTokenStatus({ sourceZip: selectedFile.sourceZip, sourcePlistPath: selectedFile.innerPath, sourceCookiePath: null, tokenPreview: "", odinTtPreview: "", status: "error", validEndpointCount: 0, endpoints: [], functions: [], error: String(error) }); });
    return () => { cancelled = true; };
  }, [selectedAppType, selectedFile?.parameterScope, selectedFile?.sourceZip, selectedFile?.innerPath]);

  useEffect(() => {
    if (selectedAppType !== "douyin" || selectedFile?.parameterScope !== "preferences" || !selectedFile.sourceZip) {
      setDouyinAccountCredentials(null);
      return;
    }
    let cancelled = false;
    setDouyinAccountCredentials({
      sourceZip: selectedFile.sourceZip,
      sourcePlistPath: selectedFile.innerPath,
      sourceCookiePath: null,
      currentSessionIdPreview: "",
      currentTokenPreview: "",
      currentOdinTtPreview: "",
      accountCount: 0,
      accounts: [],
      status: "loading",
      error: null,
    });
    void invoke<DouyinAccountCredentialResult>("extract_douyin_account_credentials", { zipPath: selectedFile.sourceZip })
      .then((result) => { if (!cancelled) setDouyinAccountCredentials(result); })
      .catch((error) => {
        if (!cancelled) {
          setDouyinAccountCredentials({
            sourceZip: selectedFile.sourceZip,
            sourcePlistPath: selectedFile.innerPath,
            sourceCookiePath: null,
            currentSessionIdPreview: "",
            currentTokenPreview: "",
            currentOdinTtPreview: "",
            accountCount: 0,
            accounts: [],
            status: "error",
            error: String(error),
          });
        }
      });
    return () => { cancelled = true; };
  }, [selectedAppType, selectedFile?.parameterScope, selectedFile?.sourceZip, selectedFile?.innerPath]);

  useEffect(() => {
    if (selectedAppType !== "douyin" || selectedFile?.parameterScope !== "preferences" || !selectedFile.sourceZip) {
      setDouyinRequestParams(null);
      return;
    }
    let cancelled = false;
    setDouyinRequestParams({ sourceZip: selectedFile.sourceZip, sourcePlistPath: selectedFile.innerPath, sourceCookiePath: null, secUserId: "", cookieHeader: "", headerCount: 0, headerText: "", headers: {}, status: "loading", error: null });
    void invoke<DouyinRequestParamsResult>("extract_douyin_request_params", { zipPath: selectedFile.sourceZip })
      .then((result) => { if (!cancelled) setDouyinRequestParams({ ...result, status: "ok", error: null }); })
      .catch((error) => { if (!cancelled) setDouyinRequestParams({ sourceZip: selectedFile.sourceZip, sourcePlistPath: selectedFile.innerPath, sourceCookiePath: null, secUserId: "", cookieHeader: "", headerCount: 0, headerText: "", headers: {}, status: "error", error: String(error) }); });
    return () => { cancelled = true; };
  }, [selectedAppType, selectedFile?.parameterScope, selectedFile?.sourceZip, selectedFile?.innerPath]);

  useEffect(() => {
    if (selectedAppType !== "toutiao" || selectedFile?.parameterScope !== "preferences" || !toutiaoPreferenceSummary?.ttUid) {
      setToutiaoSecuid(null);
      setToutiaoCertificationStatus(null);
      return;
    }
    let cancelled = false;
    setToutiaoSecuid({ ttUid: toutiaoPreferenceSummary.ttUid, ttSecuid: "", status: "loading", error: null });
    setStatus("正在联网换取头条 tt_secuid...");
    void invoke<ToutiaoSecuidResult>("resolve_toutiao_secuid", { ttUid: toutiaoPreferenceSummary.ttUid })
      .then((result) => {
        if (cancelled) return;
        setToutiaoSecuid(result);
        setStatus(result.status === "ok" ? "已自动联网换取头条 tt_secuid" : `头条 tt_secuid 换取失败：${result.error || result.status}`);
      })
      .catch((error) => { if (cancelled) return; setToutiaoSecuid({ ttUid: toutiaoPreferenceSummary.ttUid, ttSecuid: "", status: "error", error: String(error) }); setStatus(`头条 tt_secuid 换取失败：${String(error)}`); });
    return () => { cancelled = true; };
  }, [toutiaoPreferenceSummary?.ttUid, selectedAppType, selectedFile?.parameterScope]);

  useEffect(() => {
    if (selectedAppType !== "toutiao" || selectedFile?.parameterScope !== "preferences" || !selectedFile.sourceZip) {
      setToutiaoCertificationStatus(null);
      return;
    }
    let cancelled = false;
    setToutiaoCertificationStatus({ sourceZip: selectedFile.sourceZip, sourcePlistPath: null, sourceCookiePath: null, actToken: "", odinTt: "", isVerified: null, status: "loading", error: null });
    void invoke<ToutiaoCertificationStatusResult>("check_toutiao_certification_status", { zipPath: selectedFile.sourceZip })
      .then((result) => { if (!cancelled) setToutiaoCertificationStatus(result); })
      .catch((error) => { if (!cancelled) setToutiaoCertificationStatus({ sourceZip: selectedFile.sourceZip, sourcePlistPath: null, sourceCookiePath: null, actToken: "", odinTt: "", isVerified: null, status: "error", error: String(error) }); });
    return () => { cancelled = true; };
  }, [selectedAppType, selectedFile?.parameterScope, selectedFile?.sourceZip]);

  useEffect(() => {
    if (!hasTauriRuntime()) return;
    let mounted = true;
    let detach: (() => void) | null = null;
    void listen<ScanProgressPayload>("scan-progress", (event) => {
      if (!mounted) return;
      setScanProgress(event.payload);
      if (event.payload.stage === "scan_path") setStatus(event.payload.message);
    }).then((unlisten) => { if (!mounted) { unlisten(); return; } detach = unlisten; })
      .catch((error) => { if (mounted) console.warn("无法监听扫描进度事件", error); });
    return () => { mounted = false; detach?.(); };
  }, []);

  useEffect(() => {
    if (!hasTauriRuntime()) return;
    let mounted = true;
    let detach: (() => void) | null = null;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (!mounted) return;
        if (event.payload.type === "enter" || event.payload.type === "over") { setIsDragOver(true); return; }
        if (event.payload.type === "leave") { setIsDragOver(false); return; }
        const droppedZipPaths = filterZipDropPaths(event.payload.paths);
        setIsDragOver(false);
        if (!droppedZipPaths.length) { setStatus("拖入失败：仅支持 ZIP 文件"); return; }
        const nextSourcePath = droppedZipPaths.join("\n");
        setSourcePath(nextSourcePath);
        setStatus(droppedZipPaths.length === 1 ? "已拖入 1 个 ZIP，开始扫描" : `已拖入 ${droppedZipPaths.length} 个 ZIP，开始批量扫描`);
        void handleScan(nextSourcePath);
      })
      .then((unlisten) => { if (!mounted) { unlisten(); return; } detach = unlisten; })
      .catch((error) => { if (mounted) console.warn("无法启用拖拽导入", error); });
    return () => { mounted = false; detach?.(); };
  }, []);

  useEffect(() => {
    if (!filteredSqliteTables.length) { setSelectedTableName(""); return; }
    setSelectedTableName((current) => {
      if (current && filteredSqliteTables.some((table) => table.name === current)) return current;
      return filteredSqliteTables[0].name;
    });
  }, [filteredSqliteTables]);

  useEffect(() => {
    if (!selectedBatchRowKey) return;
    if (batchRows.some((row) => row.key === selectedBatchRowKey)) return;
    setSelectedBatchRowKey("");
    setIsDetailModalOpen(false);
    setActiveDetailTab("overview");
  }, [batchRows, selectedBatchRowKey]);

  async function handleScan(inputOverride?: string) {
    if (!tauriReady) {
      setStatus("当前是浏览器预览，扫描和解析请在 Tauri 桌面应用中运行");
      return;
    }
    const trimmedPath = (inputOverride ?? sourcePath).trim();
    if (!trimmedPath) return;
    setLoading(true);
    setStatus("正在扫描路径中的 ZIP...");
    setSelectedAppKey("");
    setFiles([]);
    setSelectedFile(null);
    setParseResult(null);
    setSqliteTableFilter("");
    setSqliteRowSearch("");
    setDouyinUniqueId(null);
    setDouyinPasswordStatus(null);
    setDouyinCertificationStatus(null);
    setDouyinTokenStatus(null);
    setDouyinAccountCredentials(null);
    setDouyinRequestParams(null);
    setToutiaoSecuid(null);
    setToutiaoCertificationStatus(null);
    batchRowsRef.current = [];
    setBatchRows([]);
    setBatchElapsedMs(0);
    setBatchStartedAt(null);
    batchStopRef.current = false;
    setScanProgress({ stage: "scan_path", message: "正在准备扫描路径...", current: 0, total: 1, currentZip: null, percent: 0 });
    try {
      await waitForUiFrame();
      const summary = await invoke<ZipScanSummary>("scan_path", { inputPath: trimmedPath });
      setScanSummary(summary);
      setSourcePath(summary.sourcePath);
      setIsScanModalOpen(false);
      const trackedCount = countTrackedApps(summary.apps);
      setStatus(
        summary.cacheHit
          ? `扫描完成，命中缓存，共扫描 ${summary.zipCount} 个 ZIP，命中 ${trackedCount} 个目标 APP`
          : `扫描完成，共扫描 ${summary.zipCount} 个 ZIP，命中 ${trackedCount} 个目标 APP`,
      );
      const firstTrackedApp = summary.apps.find((app) => getTrackedAppType(app.appId) !== null);
      if (firstTrackedApp) await handleLoadFiles(firstTrackedApp, summary);
    } catch (error) {
      setScanSummary(null);
      setStatus(`扫描失败：${String(error)}`);
    } finally {
      setScanProgress((current) =>
        current ? { ...current, message: current.percent >= 100 ? current.message : "扫描已结束", percent: current.percent >= 100 ? current.percent : 100, current: current.total } : null,
      );
      setLoading(false);
    }
  }

  async function handleLoadFiles(app: AppSummary, summary = scanSummary) {
    if (!tauriReady) {
      setStatus("当前是浏览器预览，候选文件加载请在 Tauri 桌面应用中运行");
      return;
    }
    if (!summary) return;
    setLoading(true);
    setSelectedAppKey(buildAppKey(app));
    setSelectedFile(null);
    setParseResult(null);
    setSqliteTableFilter("");
    setSqliteRowSearch("");
    setDouyinUniqueId(null);
    setDouyinPasswordStatus(null);
    setDouyinCertificationStatus(null);
    setDouyinTokenStatus(null);
    setDouyinAccountCredentials(null);
    setDouyinRequestParams(null);
    setToutiaoSecuid(null);
    setToutiaoCertificationStatus(null);
    setStatus(`正在加载 ${app.appId} 的候选文件...`);
    try {
      const nextFiles = await invoke<CandidateFile[]>("list_files", { zipPath: app.sourceZip, appId: app.appId });
      setFiles(nextFiles);
      setStatus(`已加载 ${app.appId} 的 ${nextFiles.length} 个候选文件，来源 ${formatBaseName(app.sourceZip)}`);
    } catch (error) {
      setFiles([]);
      setStatus(`加载文件失败：${String(error)}`);
    } finally {
      setLoading(false);
    }
  }

  async function handleParseFile(file: CandidateFile) {
    if (!tauriReady) {
      setStatus("当前是浏览器预览，文件解析请在 Tauri 桌面应用中运行");
      return;
    }
    setLoading(true);
    setSelectedFile(file);
    setParseResult(null);
    setSqliteTableFilter("");
    setSqliteRowSearch("");
    setDouyinRequestParams(null);
    setToutiaoSecuid(null);
    setDouyinPasswordStatus(null);
    setDouyinCertificationStatus(null);
    setDouyinTokenStatus(null);
    setDouyinAccountCredentials(null);
    setToutiaoCertificationStatus(null);
    setStatus(`正在解析 ${file.innerPath} ...`);
    try {
      const result = await invoke<ParseResult>("parse_file", { zipPath: file.sourceZip, innerPath: file.innerPath });
      setParseResult(result);
      const cacheHit = Boolean(result.meta?.cacheHit);
      setStatus(
        result.error
          ? `解析完成，状态：${result.parseStatus}${cacheHit ? "，命中缓存" : ""}，存在错误提示`
          : `解析完成，状态：${result.parseStatus}${cacheHit ? "，命中缓存" : ""}`,
      );
    } catch (error) {
      setParseResult(null);
      setStatus(`解析失败：${String(error)}`);
    } finally {
      setLoading(false);
    }
  }

  async function handlePickPath(mode: "directory" | "zip") {
    if (!tauriReady) {
      setStatus("当前是浏览器预览，系统选取器请在 Tauri 桌面应用中运行");
      return;
    }
    const selection = await open(
      mode === "directory"
        ? { directory: true, multiple: false, title: "选择包含 ZIP 的目录" }
        : { directory: false, multiple: false, title: "选择单个 ZIP 文件", filters: [{ name: "ZIP", extensions: ["zip"] }] },
    );
    if (typeof selection === "string" && selection.trim()) {
      setSourcePath(selection);
      setStatus(mode === "directory" ? "已选择目录路径" : "已选择 ZIP 路径");
    }
  }

  async function handleResolveToutiaoSecuid() {
    if (!tauriReady) {
      setStatus("当前是浏览器预览，联网换取 tt_secuid 请在 Tauri 桌面应用中运行");
      return;
    }
    if (!toutiaoPreferenceSummary?.ttUid) return;
    setToutiaoSecuid({ ttUid: toutiaoPreferenceSummary.ttUid, ttSecuid: "", status: "loading", error: null });
    try {
      const result = await invoke<ToutiaoSecuidResult>("resolve_toutiao_secuid", { ttUid: toutiaoPreferenceSummary.ttUid });
      setToutiaoSecuid(result);
      setStatus(result.status === "ok" ? "已联网换取头条 tt_secuid" : `头条 tt_secuid 换取失败：${result.error || result.status}`);
    } catch (error) {
      setToutiaoSecuid({ ttUid: toutiaoPreferenceSummary.ttUid, ttSecuid: "", status: "error", error: String(error) });
      setStatus(`头条 tt_secuid 换取失败：${String(error)}`);
    }
  }

  async function handleRunBatchDetection(platform: DetectionPlatform) {
    if (!tauriReady) {
      setStatus("当前是浏览器预览，批量检测请在 Tauri 桌面应用中运行");
      return;
    }
    if (!trackedApps.length || batchRunning) return;
    const runOptions = buildBatchDetectionOptions(platform, douyinOptions, toutiaoOptions);
    const initialRows = buildInitialBatchRows(trackedApps, platform);
    if (!initialRows.length) {
      setStatus(platform === "douyin" ? "当前扫描结果中没有抖音 APP" : "当前扫描结果中没有今日头条 APP");
      return;
    }
    const runStartedAt = Date.now();
    batchRowsRef.current = initialRows;
    setBatchRows(initialRows);
    setBatchRunning(true);
    setBatchElapsedMs(0);
    setBatchStartedAt(runStartedAt);
    batchStopRef.current = false;
    batchFlushPending.current = false;
    setStatus(platform === "douyin" ? "抖音批量检测开始..." : "今日头条批量检测开始...");
    let nextRowIndex = 0;
    const cpuCores = navigator.hardwareConcurrency ?? 4;
    const maxConcurrency = Math.min(20, Math.max(4, cpuCores * 2));
    const workerCount = Math.min(initialRows.length, maxConcurrency);
    setStatus(`批量检测开始，包数 ${initialRows.length}，并发 ${workerCount} 路...`);
    async function runWorker() {
      while (!batchStopRef.current) {
        const row = initialRows[nextRowIndex];
        nextRowIndex += 1;
        if (!row) return;
        const startedAt = performance.now();
        patchBatchRow(row.key, { status: "checking", fullParams: row.appType === "douyin" ? "提取中" : row.fullParams, error: null });
        try {
          const nextRows = await runBatchDetectionForRow(row, runOptions);
          const durationMs = Math.round(performance.now() - startedAt);
          const rowsWithDuration = nextRows.map((r, i) => ({
            ...r,
            durationMs,
            key: i === 0 ? r.key : `${r.key}-${i}`,
          }));
          replaceBatchRowWithMultiple(row.key, rowsWithDuration);
        } catch (error) {
          const durationMs = Math.round(performance.now() - startedAt);
          patchBatchRow(row.key, { status: "failed", durationMs, error: String(error), limitedFunctions: "检测失败" });
        }
      }
    }
    await Promise.all(Array.from({ length: workerCount }, () => runWorker()));
    if (batchStopRef.current) {
      batchRowsRef.current = batchRowsRef.current.map((item) =>
        item.status === "pending" ? { ...item, status: "skipped" } : item,
      );
    }
    setBatchRows([...batchRowsRef.current]);
    setBatchElapsedMs(Date.now() - runStartedAt);
    setBatchRunning(false);
    setStatus(batchStopRef.current ? "批量检测已停止" : "批量检测完成");
  }

  function handleStopBatchDetection() {
    batchStopRef.current = true;
    setStatus("正在停止批量检测，当前接口返回后结束...");
  }

  function handleClearBatchRows() {
    if (batchRunning) return;
    batchRowsRef.current = [];
    setBatchRows([]);
    setBatchElapsedMs(0);
    setBatchStartedAt(null);
  }

  function handleExportBatchRows(filter: "all" | "online" | "offline") {
    const rows = batchRows.filter((row) => {
      if (filter === "online") return row.status === "online";
      if (filter === "offline") return row.status === "offline" || row.status === "failed";
      return true;
    });
    if (!rows.length) { setStatus("当前没有可导出的批量检测数据"); return; }
    const headers = [
      "序号", "状态", "APP", "Token状态", "Token", "密码状态", "儿童锁", "实名状态",
      "aid", "账号", "绑定", "头条", "头条platform_screen_name", "QQ", "QQplatform_screen_name", "谷歌", "谷歌platform_screen_name",
      "ID", "IDplatform_screen_name", "微信", "微信platform_screen_name", "sec_uid", "uid", "unique_id", "手机号", "注册时间", "作品数", "关注数", "点赞数",
      "正常功能", "限制功能", "用时(ms)", "来源ZIP", "错误",
    ];
    const csvRows = rows.map((row, index) => {
      const tokenMatch = row.fullParams?.match(/x-tt-token=([^;\n\r]+)/);
      const tokenStr = tokenMatch ? tokenMatch[1] : "";
      return [
        index + 1,
        row.status,
        row.appName,
        row.tokenStatus,
        tokenStr,
        row.passwordStatus,
        row.childLockStatus,
        row.certificationStatus,
        row.aid,
        row.accountName,
        row.bindingSummary,
        row.toutiaoBinding,
        row.toutiaoPlatformScreenName,
        row.qqBinding,
        row.qqPlatformScreenName,
        row.googleBinding,
        row.googlePlatformScreenName,
        row.appleIdBinding,
        row.appleIdPlatformScreenName,
        row.wechatBinding,
        row.wechatPlatformScreenName,
        row.secUid,
        row.uid,
        row.uniqueId,
        row.phoneNumber,
        row.registerTime,
        row.awemeCount,
        row.followingCount,
        row.likedCount,
        row.normalFunctions,
        row.limitedFunctions,
        row.durationMs ?? "",
        formatBaseName(row.sourceZip),
        row.error ?? "",
      ];
    });
    const csv = [headers, ...csvRows].map((row) => row.map(escapeCsvCell).join(",")).join("\n");
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "ios-zen-batch-detection.csv";
    link.click();
    URL.revokeObjectURL(url);
    setStatus(`已导出批量检测 CSV：${rows.length} 行`);
  }

  function handleOpenScanModal(opener: HTMLElement) {
    scanDialog.rememberOpener(opener);
    setIsScanModalOpen(true);
  }

  function handleCloseScanModal() {
    if (loading) return;
    setIsScanModalOpen(false);
  }

  function handleCloseDetailModal() {
    setIsDetailModalOpen(false);
    setActiveDetailTab("overview");
  }

  function handleOpenBatchRowDetail(row: BatchDetectionRow, opener: HTMLElement) {
    detailDialog.rememberOpener(opener);
    setSelectedBatchRowKey(row.key);
    setIsDetailModalOpen(true);
    setActiveDetailTab("overview");
    const targetApp = trackedApps.find((app) => buildAppKey(app) === row.key);
    if (targetApp && buildAppKey(targetApp) !== selectedAppKey) {
      void handleLoadFiles(targetApp);
    }
  }

  function handleSelectDetailFile(file: CandidateFile) {
    setActiveDetailTab("result");
    void handleParseFile(file);
  }

  function renderParseContent() {
    if (!parseResult) {
      return <p className="empty-state">点击候选文件后在这里查看解析结果。</p>;
    }

    return (
      <>
        <div className="result-meta">
          <div><span>来源 ZIP</span><strong>{formatBaseName(parseResult.sourceZip)}</strong></div>
          <div><span>文件</span><strong title={parseResult.innerPath}>{formatCompactPath(parseResult.innerPath, 4)}</strong></div>
          <div><span>类型</span><strong>{parseResult.fileType}</strong></div>
          <div><span>状态</span><strong>{parseResult.parseStatus}</strong></div>
        </div>
        {parseResult.error ? <p className="error-text">{parseResult.error}</p> : null}
        {selectedFile?.parameterScope === "cookies" && cookieSummary ? (
          <section className="special-layout">
            <div className="special-summary-grid">
              <article className="special-summary-card"><span>APP</span><strong>{selectedAppType === "douyin" ? "抖音 Cookie" : "今日头条 Cookie"}</strong></article>
              <article className="special-summary-card"><span>sessionid</span><strong>{maskSecretValue(cookieSummary.sessionId) || "-"}</strong></article>
              <article className="special-summary-card"><span>Cookie 数量</span><strong>{cookieSummary.cookieCount}</strong></article>
              <article className="special-summary-card"><span>Cookie 长度</span><strong>{cookieSummary.cookieHeader.length}</strong></article>
            </div>
            <details className="special-raw"><summary>查看拼接后的 Cookie</summary><pre>{cookieSummary.cookieHeader || "-"}</pre></details>
            {cookiePreviewItems.length ? (
              <div className="cookie-grid">
                {cookiePreviewItems.map((cookie, index) => (
                  <article key={`${cookie.name}-${cookie.domain}-${index}`} className="cookie-card">
                    <div className="cookie-card-header"><strong>{cookie.name || "(未命名 Cookie)"}</strong><span>{cookie.domain || "-"}</span></div>
                    <div className="cookie-card-body">
                      <span>Path: {cookie.path || "/"}</span>
                      <span>Expires: {cookie.expiresLabel || "Session"}</span>
                      {"createdLabel" in cookie && cookie.createdLabel ? <span>Created: {cookie.createdLabel}</span> : null}
                      {"flagsLabel" in cookie && cookie.flagsLabel ? <span>Flags: {cookie.flagsLabel}</span> : null}
                      <span className="cookie-value">Value: {cookie.value || "-"}</span>
                    </div>
                  </article>
                ))}
              </div>
            ) : <p className="empty-state">这个 Cookie 文件暂时没有解析出可展示条目。</p>}
          </section>
        ) : selectedAppType === "douyin" && selectedFile?.parameterScope === "preferences" && douyinPreferenceSummary ? (
          <section className="special-layout">
            <div className="special-summary-grid">
              <article className="special-summary-card"><span>抖音 UID</span><strong>{douyinPreferenceSummary.dyUid || "-"}</strong></article>
              <article className="special-summary-card"><span>抖音 UQID</span><strong>{douyinUniqueId?.status === "loading" ? "查询中" : douyinUniqueId?.uniqueId || "-"}</strong></article>
              <article className="special-summary-card"><span>抖音 SecUid</span><strong>{douyinPreferenceSummary.dySecUid || "-"}</strong></article>
              <article className="special-summary-card"><span>缓存对象</span><strong>{douyinPreferenceSummary.hasUserStorageCache ? "存在" : "无"}</strong></article>
              <article className="special-summary-card"><span>密码状态</span><strong>{formatDouyinPasswordLabel(douyinPasswordStatus)}</strong></article>
              <article className="special-summary-card"><span>实名状态</span><strong>{formatDouyinCertificationLabel(douyinCertificationStatus)}</strong></article>
              <article className="special-summary-card"><span>Token 检测</span><strong>{formatDouyinTokenLabel(douyinTokenStatus)}</strong></article>
              <article className="special-summary-card"><span>通过接口</span><strong>{douyinTokenStatus?.status === "loading" ? "检测中" : douyinTokenStatus ? douyinTokenStatus.validEndpointCount + " / " + (douyinTokenStatus.endpoints.length || 2) : "-"}</strong></article>
              <article className="special-summary-card"><span>检测账号</span><strong>{douyinPasswordStatus?.accountName || douyinCertificationStatus?.accountName || douyinTokenStatus?.endpoints.find((endpoint) => endpoint.nickname)?.nickname || "-"}</strong></article>
              <article className="special-summary-card"><span>Token 尾号</span><strong>{douyinTokenStatus?.tokenPreview || "-"}</strong></article>
              <article className="special-summary-card"><span>命中来源</span><strong>{douyinPreferenceSummary.hitSource || "-"}</strong></article>
            </div>
            <div className="special-summary-grid">
              <article className="special-summary-card"><span>当前 Session</span><strong>{douyinAccountCredentials?.status === "loading" ? "读取中" : douyinAccountCredentials?.currentSessionIdPreview || "-"}</strong></article>
              <article className="special-summary-card"><span>当前 Token</span><strong>{douyinAccountCredentials?.status === "loading" ? "读取中" : douyinAccountCredentials?.currentTokenPreview || "-"}</strong></article>
              <article className="special-summary-card"><span>当前 odin_tt</span><strong>{douyinAccountCredentials?.status === "loading" ? "读取中" : douyinAccountCredentials?.currentOdinTtPreview || "-"}</strong></article>
              <article className="special-summary-card"><span>多账号数量</span><strong>{douyinAccountCredentials?.status === "loading" ? "读取中" : douyinAccountCredentials?.accountCount ?? "-"}</strong></article>
            </div>
            {douyinAccountCredentials?.accounts.length ? (
              <div className="preferences-grid">
                {douyinAccountCredentials.accounts.map((account) => (
                  <article key={account.uid} className="preference-card preference-card-wide">
                    <div className="preference-card-header">
                      <strong>{account.nickname || account.uid || "未命名账号"}</strong>
                      <span>{account.uniqueId || account.shortId || "无 unique_id"}</span>
                    </div>
                    <div className="preference-card-body">
                      <code>{[
                        `uid: ${account.uid || "-"}`,
                        `sec_uid: ${account.secUid || "-"}`,
                        `session: ${account.sessionIdPreview || "-"}`,
                        `token: ${account.accessTokenPreview || "-"}`,
                        `open_id: ${account.openIdPreview || "-"}`,
                        account.authTimeLabel ? `auth_time: ${account.authTimeLabel}` : "",
                      ].filter(Boolean).join("\n")}</code>
                    </div>
                  </article>
                ))}
              </div>
            ) : null}
            <div className="preferences-grid">
              {buildSpecialEntries([
                ["AWEUserStorageCacheUserKey", douyinPreferenceSummary.rawCacheValue],
                ["ABTestCurrentUserKey", douyinPreferenceSummary.abTestCurrentUserKey],
                ["dy_uqid", douyinUniqueId?.uniqueId],
                ["kTTAccountTicketGuardSecUserIdTsSignDic", douyinPreferenceSummary.guardSecUid],
                ["命中 profile uid", douyinPreferenceSummary.profileUid],
                ["命中 MS4 secuid", douyinPreferenceSummary.matchedSecUid],
              ]).map((entry) => (
                <article key={entry.path} className="preference-card">
                  <div className="preference-card-header"><strong>{entry.path}</strong><span>{entry.valueType}</span></div>
                  <div className="preference-card-body"><code>{entry.value}</code></div>
                </article>
              ))}
            </div>
            {douyinUniqueId?.error ? <p className="empty-state">dy_uqid 查询失败：{douyinUniqueId.error}</p> : null}
            {douyinPasswordStatus?.error ? <p className="empty-state">抖音密码状态检测失败：{douyinPasswordStatus.error}</p> : null}
            {douyinCertificationStatus?.error ? <p className="empty-state">抖音实名状态检测失败：{douyinCertificationStatus.error}</p> : null}
            {douyinTokenStatus?.error ? <p className="empty-state">抖音 Token 检测提示：{douyinTokenStatus.error}</p> : null}
            {douyinAccountCredentials?.error ? <p className="empty-state">抖音多账号凭证读取提示：{douyinAccountCredentials.error}</p> : null}
            {douyinTokenStatus?.endpoints.length ? (
              <div className="preferences-grid">
                {douyinTokenStatus.endpoints.map((endpoint) => (
                  <article key={endpoint.name} className="preference-card">
                    <div className="preference-card-header"><strong>{formatDouyinEndpointName(endpoint.name)}</strong><span>{formatDouyinEndpointStatus(endpoint)}</span></div>
                    <div className="preference-card-body">
                      <code>{["http: " + (endpoint.httpStatus ?? "-"), "status_code: " + (endpoint.statusCode ?? "-"), "uid: " + (endpoint.uid || "-"), "sec_uid: " + (endpoint.secUid || "-"), "nickname: " + (endpoint.nickname || "-"), endpoint.message ? "message: " + endpoint.message : ""].filter(Boolean).join("\n")}</code>
                    </div>
                  </article>
                ))}
              </div>
            ) : null}
            <div className="special-summary-grid">
              <article className="special-summary-card"><span>全参状态</span><strong>{douyinRequestParams?.status === "loading" ? "提取中" : douyinRequestParams?.status === "ok" ? "已生成" : douyinRequestParams?.status === "error" ? "失败" : "-"}</strong></article>
            </div>
            {douyinRequestParams?.error ? <p className="empty-state">抖音全参提取失败：{douyinRequestParams.error}</p> : null}
            {douyinRequestParams ? (
              <div className="preferences-grid">
                <article className="preference-card">
                  <div className="preference-card-header"><strong>序号</strong><span>index</span></div>
                  <div className="preference-card-body"><code>{selectedAppOrderLabel || "-"}</code></div>
                </article>
                <article className="preference-card preference-card-wide">
                  <div className="preference-card-header"><strong>全参</strong><span>readdyreqparams</span></div>
                  <div className="preference-card-body"><code>{douyinRequestParams.headerText || "-"}</code></div>
                </article>
              </div>
            ) : null}
            <details className="special-raw"><summary>查看原始 JSON</summary><pre>{JSON.stringify(parseResult.parsedData, null, 2)}</pre></details>
          </section>
        ) : selectedAppType === "toutiao" && selectedFile?.parameterScope === "preferences" && toutiaoPreferenceSummary ? (
          <section className="special-layout">
            <div className="special-summary-grid">
              <article className="special-summary-card"><span>头条 Token</span><strong>{maskSecretValue(toutiaoPreferenceSummary.ttToken) || "-"}</strong></article>
              <article className="special-summary-card"><span>头条 UID</span><strong>{toutiaoPreferenceSummary.ttUid || "-"}</strong></article>
              <article className="special-summary-card"><span>UID 来源</span><strong>{toutiaoPreferenceSummary.uidSource || "-"}</strong></article>
              <article className="special-summary-card"><span>tt_secuid</span><strong>{toutiaoSecuid?.status === "loading" ? "换取中" : toutiaoSecuid?.ttSecuid || "-"}</strong></article>
              <article className="special-summary-card"><span>实名状态</span><strong>{formatToutiaoCertificationLabel(toutiaoCertificationStatus)}</strong></article>
              <article className="special-summary-card"><span>odin_tt</span><strong>{maskSecretValue(toutiaoCertificationStatus?.odinTt) || "-"}</strong></article>
            </div>
            <div className="special-actions">
              <button className="secondary-button inline-button" onClick={() => void handleResolveToutiaoSecuid()} disabled={!toutiaoPreferenceSummary.ttUid || toutiaoSecuid?.status === "loading"}>重新联网换取 tt_secuid</button>
            </div>
            <div className="preferences-grid">
              {buildSpecialEntries([
                ["bdaccount_session_x_tt_token", maskSecretValue(toutiaoPreferenceSummary.ttToken)],
                ["tt_secuid", toutiaoSecuid?.ttSecuid],
                ["ABTestCurrentUserKey", toutiaoPreferenceSummary.abTestCurrentUserKey],
                ["kTTAccountOAuthTokenInfoStorageKey[0].userId", toutiaoPreferenceSummary.oauthUserId],
              ]).map((entry) => (
                <article key={entry.path} className="preference-card">
                  <div className="preference-card-header"><strong>{entry.path}</strong><span>{entry.valueType}</span></div>
                  <div className="preference-card-body"><code>{entry.value}</code></div>
                </article>
              ))}
            </div>
            {toutiaoSecuid?.error ? <p className="empty-state">tt_secuid 换取失败：{toutiaoSecuid.error}</p> : null}
            {toutiaoCertificationStatus?.error ? <p className="empty-state">头条实名状态检测失败：{toutiaoCertificationStatus.error}</p> : null}
            <details className="special-raw"><summary>查看原始 JSON</summary><pre>{JSON.stringify(parseResult.parsedData, null, 2)}</pre></details>
          </section>
        ) : parseResult.fileType === "sqlite" && selectedSqliteTable ? (
          <section className="sqlite-layout">
            <aside className="sqlite-table-list">
              <div className="sqlite-section-title"><span>数据表</span><strong>{filteredSqliteTables.length}</strong></div>
              <input className="sqlite-search-input" value={sqliteTableFilter} onChange={(event) => setSqliteTableFilter(event.currentTarget.value)} placeholder="搜索数据表" />
              {filteredSqliteTables.map((table) => (
                <button key={table.name} className={`sqlite-table-button ${selectedSqliteTable.name === table.name ? "active" : ""}`} onClick={() => setSelectedTableName(table.name)}>
                  <span>{table.name}</span>
                  <small>{table.rows.length} 行预览</small>
                </button>
              ))}
            </aside>
            <div className="sqlite-table-detail">
              <div className="sqlite-section-title"><span>当前表</span><strong>{selectedSqliteTable.name}</strong></div>
              <input className="sqlite-search-input" value={sqliteRowSearch} onChange={(event) => setSqliteRowSearch(event.currentTarget.value)} placeholder="表内搜索字段值" />
              <div className="sqlite-columns">
                {selectedSqliteTable.columns.map((column) => (
                  <span key={`${selectedSqliteTable.name}-${column.name}`}>{column.name}{column.dataType ? ` (${column.dataType})` : ""}</span>
                ))}
              </div>
              {filteredSqliteRows.length ? (
                <div className="sqlite-rows"><pre>{JSON.stringify(filteredSqliteRows, null, 2)}</pre></div>
              ) : <p className="empty-state">当前筛选条件下没有可预览数据。</p>}
            </div>
          </section>
        ) : selectedFile?.parameterScope === "preferences" && preferenceEntries.length ? (
          <section className="preferences-layout">
            <div className="preferences-header"><span>参数项</span><strong>{preferenceEntries.length}</strong></div>
            <div className="preferences-grid">
              {preferenceEntries.map((entry) => (
                <article key={entry.path} className="preference-card">
                  <div className="preference-card-header"><strong>{entry.path}</strong><span>{entry.valueType}</span></div>
                  <div className="preference-card-body"><code>{entry.value}</code></div>
                </article>
              ))}
            </div>
            <details className="preferences-raw"><summary>查看原始 JSON</summary><pre>{JSON.stringify(parseResult.parsedData, null, 2)}</pre></details>
          </section>
        ) : (
          <pre>{JSON.stringify(parseResult.parsedData, null, 2)}</pre>
        )}
      </>
    );
  }

  function renderDetailOverview() {
    if (!selectedBatchRow) {
      return <p className="empty-state">当前没有可展示的检测详情。</p>;
    }

    const cards = [
      ["APP", selectedBatchRow.appName],
      ["状态", formatBatchRowStatus(selectedBatchRow.status)],
      ["账号", selectedBatchRow.accountName || "-"],
      ["绑定", selectedBatchRow.bindingSummary || "-"],
      ["头条", selectedBatchRow.toutiaoBinding || "-"],
      ["头条platform_screen_name", selectedBatchRow.toutiaoPlatformScreenName || "-"],
      ["QQ", selectedBatchRow.qqBinding || "-"],
      ["QQplatform_screen_name", selectedBatchRow.qqPlatformScreenName || "-"],
      ["谷歌", selectedBatchRow.googleBinding || "-"],
      ["谷歌platform_screen_name", selectedBatchRow.googlePlatformScreenName || "-"],
      ["ID", selectedBatchRow.appleIdBinding || "-"],
      ["IDplatform_screen_name", selectedBatchRow.appleIdPlatformScreenName || "-"],
      ["微信", selectedBatchRow.wechatBinding || "-"],
      ["微信platform_screen_name", selectedBatchRow.wechatPlatformScreenName || "-"],
      ["SecUid", selectedBatchRow.secUid || "-"],
      ["UID", selectedBatchRow.uid || "-"],
      ["UniqueId", selectedBatchRow.uniqueId || "-"],
      ["手机号", selectedBatchRow.phoneNumber || "-"],
      ["注册时间", selectedBatchRow.registerTime || "-"],
      ["AID", selectedBatchRow.aid || "-"],
      ["Token", selectedBatchRow.tokenStatus || "-"],
      ["密码状态", selectedBatchRow.passwordStatus || "-"],
      ["儿童锁", selectedBatchRow.childLockStatus || "-"],
      ["实名状态", selectedBatchRow.certificationStatus || "-"],
      ["正常功能", selectedBatchRow.normalFunctions || "-"],
      ["限制功能", selectedBatchRow.limitedFunctions || "-"],
      ["来源 ZIP", formatBaseName(selectedBatchRow.sourceZip)],
      ["用时", selectedBatchRow.durationMs == null ? "-" : `${selectedBatchRow.durationMs} ms`],
    ] as const;

    return (
      <section className="detail-overview-grid">
        {cards.map(([label, value]) => (
          <article key={label} className="detail-overview-card">
            <span>{label}</span>
            <strong>{value}</strong>
          </article>
        ))}
        <article className="detail-overview-card detail-overview-card-wide">
          <span>全参预览</span>
          <strong>{formatTextPreview(selectedBatchRow.fullParams || "-", 260)}</strong>
        </article>
        {selectedBatchRow.error ? (
          <article className="detail-overview-card detail-overview-card-wide">
            <span>错误信息</span>
            <strong>{selectedBatchRow.error}</strong>
          </article>
        ) : null}
      </section>
    );
  }

  function renderDetailFilesTab() {
    return (
      <section className="detail-files-layout">
        <div className="detail-files-list">
          <div className="detail-section-head">
            <div>
              <span>候选文件</span>
              <strong>{visibleFiles.length}</strong>
            </div>
            <small>{selectedBatchApp ? `${selectedBatchApp.displayName} · ${formatBaseName(selectedBatchApp.sourceZip)}` : "等待选择检测行"}</small>
          </div>
          {visibleFiles.length ? (
            visibleFiles.map((file) => (
              <button
                key={buildFileKey(file)}
                className={`list-item ${selectedFile && buildFileKey(selectedFile) === buildFileKey(file) ? "active" : ""}`}
                onClick={() => handleSelectDetailFile(file)}
                disabled={loading}
              >
                <span className="title" title={file.innerPath}>{formatCompactPath(file.innerPath, 4)}</span>
                <span className="sub-meta">来源 ZIP：{formatBaseName(file.sourceZip)}</span>
                <span className="meta">{formatParameterScope(file.parameterScope)} · {file.fileType} · {formatBytes(file.size)} · {file.parseSupported ? "可解析" : "暂不解析"}</span>
              </button>
            ))
          ) : (
            <p className="empty-state">当前没有可展示的候选文件。先扫描目标 ZIP，再打开一条检测记录。</p>
          )}
        </div>

        <aside className="detail-files-sidebar">
          <div className="detail-section-head">
            <div>
              <span>当前文件</span>
              <strong>{selectedFile ? formatParameterScope(selectedFile.parameterScope) : "-"}</strong>
            </div>
          </div>
          {selectedFile ? (
            <div className="detail-current-file">
              <strong title={selectedFile.innerPath}>{formatCompactPath(selectedFile.innerPath, 5)}</strong>
              <span>类型：{selectedFile.fileType}</span>
              <span>大小：{formatBytes(selectedFile.size)}</span>
              <span>解析：{selectedFile.parseSupported ? "支持" : "暂不支持"}</span>
              <button className="secondary-button inline-button" onClick={() => setActiveDetailTab("result")} disabled={loading}>
                查看解析结果
              </button>
            </div>
          ) : (
            <p className="empty-state">点击左侧候选文件后，会直接解析并跳转到“解析结果”。</p>
          )}
        </aside>
      </section>
    );
  }

  function renderRawDataTab() {
    if (!selectedFile && !parseResult) {
      return <p className="empty-state">先在“候选文件”里选择一个文件，原始数据才会显示在这里。</p>;
    }

    const rawPayload = {
      row: selectedBatchRow,
      selectedFile,
      parseResult,
      cookieSummary,
      douyinUniqueId,
      douyinPasswordStatus,
      douyinCertificationStatus,
      douyinTokenStatus,
      douyinRequestParams,
      toutiaoSecuid,
      toutiaoCertificationStatus,
    };

    return (
      <section className="detail-raw-layout">
        <div className="detail-raw-meta">
          <article className="detail-overview-card">
            <span>来源 ZIP</span>
            <strong>{selectedFile ? formatBaseName(selectedFile.sourceZip) : selectedBatchRow ? formatBaseName(selectedBatchRow.sourceZip) : "-"}</strong>
          </article>
          <article className="detail-overview-card">
            <span>文件路径</span>
            <strong>{selectedFile ? formatCompactPath(selectedFile.innerPath, 5) : "-"}</strong>
          </article>
          <article className="detail-overview-card">
            <span>解析状态</span>
            <strong>{parseResult?.parseStatus ?? "-"}</strong>
          </article>
        </div>
        <pre>{JSON.stringify(rawPayload, null, 2)}</pre>
      </section>
    );
  }

  function renderDetailContent() {
    switch (activeDetailTab) {
      case "overview":
        return renderDetailOverview();
      case "files":
        return renderDetailFilesTab();
      case "result":
        return renderParseContent();
      case "raw":
        return renderRawDataTab();
      default:
        return renderDetailOverview();
    }
  }

  return (
    <main className="app-shell app-shell-v3">
      <DetectorWorkbench
        runtimeReady={tauriReady}
        batchRows={batchRows}
        batchStats={batchStats}
        douyinOptions={douyinOptions}
        toutiaoOptions={toutiaoOptions}
        batchRunning={batchRunning}
        batchStartedAt={batchStartedAt}
        batchElapsedMs={batchElapsedMs}
        trackedAppCount={trackedApps.length}
        scanSummary={scanSummary}
        status={status}
        loading={loading}
        selectedRowKey={selectedBatchRowKey}
        onOpenScanModal={handleOpenScanModal}
        onSelectRow={handleOpenBatchRowDetail}
        onRunDetection={(platform) => void handleRunBatchDetection(platform)}
        onStopDetection={handleStopBatchDetection}
        onClearRows={handleClearBatchRows}
        onExportRows={handleExportBatchRows}
        onDouyinOptionsChange={setDouyinOptions}
        onToutiaoOptionsChange={setToutiaoOptions}
        onSetStatus={setStatus}
      />

      {isScanModalOpen ? (
        <div className="modal-backdrop" onClick={handleCloseScanModal}>
          <section
            className="scan-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="scan-dialog-title"
            tabIndex={-1}
            ref={scanDialog.dialogRef}
            onKeyDown={scanDialog.onDialogKeyDown}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="modal-header">
              <div>
                <p className="modal-kicker">扫描入口</p>
                <h2 id="scan-dialog-title">导入 ZIP 或目录</h2>
                <p>选择 ZIP 文件或目录，扫描后即可按平台开始检测。</p>
              </div>
              <button data-modal-autofocus className="modal-close" onClick={handleCloseScanModal} disabled={loading} aria-label="关闭扫描弹窗">
                关闭
              </button>
            </div>

            <label className={`field scan-modal-field ${isDragOver ? "field-drop-active" : ""}`}>
              <span>ZIP 或目录路径</span>
              <input
                value={sourcePath}
                onChange={(event) => setSourcePath(event.currentTarget.value)}
                placeholder="输入 ZIP 路径、目录路径，或直接拖入一个/多个 ZIP 文件"
              />
              <small className="field-hint">
                支持单个 ZIP、目录扫描，也支持把多个 ZIP 直接拖入窗口后自动批量扫描。
              </small>
            </label>

            <div className="scan-modal-actions">
              <button className="secondary-button" onClick={() => void handlePickPath("directory")} disabled={!tauriReady || loading}>选目录</button>
              <button className="secondary-button" onClick={() => void handlePickPath("zip")} disabled={!tauriReady || loading}>选 ZIP</button>
              <button onClick={() => void handleScan()} disabled={!tauriReady || loading || !sourcePath.trim()}>开始扫描</button>
            </div>

            {scanProgress && loading ? (
              <section className="progress-panel progress-panel-inline">
                <div className="progress-header">
                  <strong>扫描进度</strong>
                  <span>{Math.min(100, Math.max(0, scanProgress.percent))}%</span>
                </div>
                <div className="progress-bar" role="progressbar" aria-valuenow={Math.min(100, Math.max(0, scanProgress.percent))} aria-valuemin={0} aria-valuemax={100}>
                  <span className="progress-fill" style={{ width: `${Math.min(100, Math.max(0, scanProgress.percent))}%` }} />
                </div>
                <div className="progress-meta">
                  <span>{scanProgress.message}</span>
                  <span>{scanProgress.current} / {scanProgress.total}</span>
                </div>
                {scanProgress.currentZip ? <small className="progress-file">当前 ZIP：{formatBaseName(scanProgress.currentZip)}</small> : null}
              </section>
            ) : (
              <div className="scan-modal-status">
                <span className={`detector-surface-badge detector-surface-badge-${loading ? "loading" : scanSummary ? "ready" : "idle"}`}>
                  {loading ? "处理中" : scanSummary ? "已扫描" : "待扫描"}
                </span>
                <p>{tauriReady ? status : "当前窗口仅用于第三版页面预览。扫描、解析、系统选取器需要在 Tauri 桌面应用中运行。"}</p>
              </div>
            )}
          </section>
        </div>
      ) : null}

      {isDetailModalOpen && selectedBatchRow ? (
        <div className="modal-backdrop" onClick={handleCloseDetailModal}>
          <section
            className="detail-modal-window"
            role="dialog"
            aria-modal="true"
            aria-labelledby="detail-dialog-title"
            tabIndex={-1}
            ref={detailDialog.dialogRef}
            onKeyDown={detailDialog.onDialogKeyDown}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="detail-modal-header">
              <div>
                <p className="modal-kicker">检测详情</p>
                <h2 id="detail-dialog-title">{selectedBatchRow.appName} · {selectedBatchRow.accountName || selectedBatchRow.orderLabel}</h2>
                <p>{formatBaseName(selectedBatchRow.sourceZip)} · {formatBatchRowStatus(selectedBatchRow.status)}</p>
              </div>
              <button data-modal-autofocus className="modal-close" onClick={handleCloseDetailModal} aria-label="关闭详情弹窗">
                关闭
              </button>
            </div>

            <div className="detail-modal-summary">
              <article className="detail-summary-card">
                <span>来源 ZIP</span>
                <strong>{formatBaseName(selectedBatchRow.sourceZip)}</strong>
              </article>
              <article className="detail-summary-card">
                <span>检测状态</span>
                <strong>{formatBatchRowStatus(selectedBatchRow.status)}</strong>
              </article>
              <article className="detail-summary-card">
                <span>Token</span>
                <strong>{selectedBatchRow.tokenStatus}</strong>
              </article>
              <article className="detail-summary-card">
                <span>当前文件</span>
                <strong>{selectedFile ? formatCompactPath(selectedFile.innerPath, 4) : "-"}</strong>
              </article>
            </div>

            <div className="detail-tabs" role="tablist" aria-label="检测详情">
              {[
                { key: "overview" as const, label: "账号总览" },
                { key: "files" as const, label: "候选文件" },
                { key: "result" as const, label: "解析结果" },
                { key: "raw" as const, label: "原始数据" },
              ].map((tab) => (
                <button
                  key={tab.key}
                  id={`detail-tab-${tab.key}`}
                  className={`detail-tab ${activeDetailTab === tab.key ? "active" : ""}`}
                  role="tab"
                  aria-selected={activeDetailTab === tab.key}
                  aria-controls={`detail-panel-${tab.key}`}
                  onClick={() => setActiveDetailTab(tab.key)}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            <div
              id={`detail-panel-${activeDetailTab}`}
              className="detail-modal-content"
              role="tabpanel"
              aria-labelledby={`detail-tab-${activeDetailTab}`}
            >
              {renderDetailContent()}
            </div>
          </section>
        </div>
      ) : null}
    </main>
  );
}

function waitForUiFrame() {
  return new Promise<void>((resolve) => { requestAnimationFrame(() => resolve()); });
}

function formatBytes(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function formatParameterScope(scope: string) {
  switch (scope) {
    case "preferences": return "Preferences";
    case "cookies": return "Cookies";
    case "webkit": return "WebKit";
    default: return "其他";
  }
}

function formatScanMode(mode?: string | null) {
  switch (mode) {
    case "directory":
      return "目录批量";
    case "zip":
      return "单 ZIP";
    case "files":
      return "拖入批量";
    default:
      return "待扫描";
  }
}

function formatBatchRowStatus(status: BatchDetectionRow["status"]) {
  switch (status) {
    case "online":
      return "在线";
    case "offline":
      return "掉线";
    case "checking":
      return "检测中";
    case "failed":
      return "失败";
    case "skipped":
      return "已跳过";
    case "pending":
    default:
      return "待检测";
  }
}

function formatBaseName(path: string) {
  if (!path) return "-";
  const normalized = path.replace(/\\/g, "/");
  const segments = normalized.split("/").filter(Boolean);
  return segments.length ? segments[segments.length - 1] : path;
}

function formatCompactPath(path: string, keepSegments = 3) {
  if (!path) return "-";
  const normalized = path.replace(/\\/g, "/");
  const segments = normalized.split("/").filter(Boolean);
  if (segments.length <= keepSegments) return normalized;
  return `…/${segments.slice(-keepSegments).join("/")}`;
}

function formatTextPreview(value: string, maxLength: number) {
  const normalized = String(value || "").replace(/\s+/g, " ").trim();
  if (!normalized) return "-";
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, maxLength)}…`;
}

function extractSessionFromFullParams(value: string) {
  const text = String(value || "");
  const cookieLine =
    text
      .split(/\r?\n/)
      .find((line) => line.trim().toLowerCase().startsWith("cookie=")) ?? text;
  const cookieText = cookieLine.replace(/^cookie=/i, "");
  for (const key of ["sessionid", "sessionid_ss", "sid_tt"]) {
    const match = cookieText.match(new RegExp(`(?:^|;\\s*)${key}=([^;\\n\\r]+)`, "i"));
    if (match?.[1]?.trim()) return match[1].trim();
  }
  return "-";
}

function buildAppKey(app: AppSummary) {
  return `${app.sourceZip}::${app.appId}`;
}

function buildFileKey(file: CandidateFile) {
  return `${file.sourceZip}::${file.innerPath}`;
}

function countTrackedApps(apps: AppSummary[]) {
  return apps.filter((app) => getTrackedAppType(app.appId) !== null).length;
}

function getTrackedAppType(appId: string) {
  const lower = appId.toLowerCase();
  if (lower === "com.ss.iphone.ugc.aweme") return "douyin" as const;
  if (lower === "com.ss.iphone.article.news") return "toutiao" as const;
  return null;
}

function matchesTrackedFile(appType: "douyin" | "toutiao", file: CandidateFile) {
  const lower = file.innerPath.toLowerCase();
  if (appType === "douyin") {
    return lower.endsWith("/library/preferences/com.ss.iphone.ugc.aweme.plist") || lower.endsWith("/library/cookies/cookies.binarycookies");
  }
  return lower.endsWith("/library/preferences/com.ss.iphone.article.news.plist") || lower.includes("/library/cookies/");
}

export default App;

function flattenPreferenceEntries(value: unknown) {
  const entries: PreferenceEntry[] = [];
  walkPreferenceValue(value, "", entries);
  return entries.sort((left, right) => left.path.localeCompare(right.path));
}

function walkPreferenceValue(value: unknown, currentPath: string, entries: PreferenceEntry[]) {
  if (Array.isArray(value)) {
    if (!value.length) { entries.push({ path: currentPath || "(root)", value: "[]", valueType: "array" }); return; }
    value.forEach((item, index) => { const nextPath = currentPath ? `${currentPath}[${index}]` : `[${index}]`; walkPreferenceValue(item, nextPath, entries); });
    return;
  }
  if (value && typeof value === "object") {
    const objectEntries = Object.entries(value as Record<string, unknown>);
    if (!objectEntries.length) { entries.push({ path: currentPath || "(root)", value: "{}", valueType: "object" }); return; }
    objectEntries.forEach(([key, nestedValue]) => { const nextPath = currentPath ? `${currentPath}.${key}` : key; walkPreferenceValue(nestedValue, nextPath, entries); });
    return;
  }
  entries.push({ path: currentPath || "(root)", value: formatPreferenceValue(value), valueType: detectPreferenceValueType(value) });
}

function formatPreferenceValue(value: unknown) {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean" || value === null || value === undefined) return String(value);
  return JSON.stringify(value);
}

function detectPreferenceValueType(value: unknown) {
  if (Array.isArray(value)) return "array";
  if (value === null) return "null";
  return typeof value;
}

function normalizeCookieRow(row: Record<string, unknown>) {
  const name = firstString(row, ["name", "cookie_name"]);
  const domain = firstString(row, ["domain", "host", "host_key"]);
  const path = firstString(row, ["path"]);
  const value = firstString(row, ["value", "cookie_value", "encrypted_value"]);
  const expiresLabel = firstString(row, ["expires", "expiry", "expire_time", "expires_utc"]) || "";
  const createdLabel = firstString(row, ["created", "created_at", "createdLabel"]);
  const flagsLabel = firstString(row, ["flagsLabel", "is_secure", "is_httponly"]);
  if (!name && !domain && !value) return null;
  return { name, domain, path, value, expiresLabel, createdLabel, flagsLabel };
}

function getBinaryCookieItems(value: unknown) {
  const cookies = readPath(value, ["cookies"]);
  if (!Array.isArray(cookies)) return [];
  return cookies
    .map<CookiePreviewItem | null>((item) => {
      if (!item || typeof item !== "object") return null;
      const row = item as Record<string, unknown>;
      return {
        name: firstString(row, ["name"]),
        domain: firstString(row, ["domain"]),
        path: firstString(row, ["path"]),
        value: firstString(row, ["value"]),
        expiresLabel: firstString(row, ["expiresLabel", "expires"]),
        createdLabel: firstString(row, ["createdLabel", "created"]),
        flagsLabel: firstString(row, ["flagsLabel"]),
      } satisfies CookiePreviewItem;
    })
    .filter((item): item is CookiePreviewItem => Boolean(item));
}

function buildCookieSummary(value: unknown, cookies: CookiePreviewItem[]) {
  const header = firstValueString(
    readPath(value, ["cookieHeader"]),
    cookies.map((cookie) => (cookie.name && cookie.value ? `${cookie.name}=${cookie.value}` : "")).filter(Boolean).join("; "),
  );
  const sessionId =
    firstValueString(readPath(value, ["sessionId"])) ||
    cookies.find((cookie) => cookie.name.toLowerCase() === "sessionid")?.value ||
    "";
  return {
    sessionId,
    cookieHeader: header,
    cookieCount: Number(readPath(value, ["cookieCount"])) || cookies.filter((cookie) => cookie.name || cookie.value).length,
  };
}

function extractDouyinPreferenceSummary(value: unknown) {
  const allStrings = collectStringValues(value);
  const rawCacheValue = firstValueString(readPath(value, ["AWEUserStorageCacheUserKey"]));
  const abTestCurrentUserKey = firstValueString(readPath(value, ["ABTestCurrentUserKey"]));
  const guardObject = readPath(value, ["kTTAccountTicketGuardSecUserIdTsSignDic"]);
  const matchedSecUid = findFirstPattern(allStrings, /MS4[\w\-_]+/);
  const guardSecUid = firstObjectKey(guardObject) || matchedSecUid;
  const profileUid = extractProfileUid(allStrings);
  const dyUid = profileUid || abTestCurrentUserKey;
  const dySecUid = matchedSecUid || guardSecUid;
  return {
    dyUid, dySecUid, profileUid, matchedSecUid, rawCacheValue, abTestCurrentUserKey, guardSecUid,
    hasUserStorageCache: Boolean(rawCacheValue),
    hitSource: [
      profileUid ? "AWEUserStorageCacheUserKey -> profile" : "",
      matchedSecUid ? "AWEUserStorageCacheUserKey -> MS4" : "",
      abTestCurrentUserKey ? "ABTestCurrentUserKey" : "",
      guardSecUid ? "kTTAccountTicketGuardSecUserIdTsSignDic" : "",
    ].filter(Boolean).join(" / "),
  };
}

function extractToutiaoPreferenceSummary(value: unknown) {
  const abTestCurrentUserKey = firstValueString(readPath(value, ["ABTestCurrentUserKey"]));
  const ttToken = firstValueString(readPath(value, ["bdaccount_session_x_tt_token"]));
  const oauthUserId = firstValueString(readPath(value, ["kTTAccountOAuthTokenInfoStorageKey", 0, "userId"]));
  const ttUid = abTestCurrentUserKey || oauthUserId;
  return { ttToken, ttUid, uidSource: abTestCurrentUserKey ? "ABTestCurrentUserKey" : oauthUserId ? "kTTAccountOAuthTokenInfoStorageKey" : "", abTestCurrentUserKey, oauthUserId };
}

function buildSpecialEntries(entries: Array<[string, string | undefined]>) {
  return entries.filter(([, value]) => Boolean(value)).map(([path, value]) => ({ path, value: value || "-", valueType: "string" }));
}

function readPath(value: unknown, path: Array<string | number>): unknown {
  let current = value;
  for (const segment of path) {
    if (typeof segment === "number") {
      if (!Array.isArray(current)) return undefined;
      current = current[segment];
      continue;
    }
    if (!current || typeof current !== "object") return undefined;
    current = (current as Record<string, unknown>)[segment];
  }
  return current;
}

function firstValueString(value: unknown, fallback = "") {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return fallback;
}

function maskSecretValue(value: string | null | undefined) {
  const trimmed = (value || "").trim();
  if (!trimmed) return "";
  if (trimmed.length <= 14) return "***";
  return `${trimmed.slice(0, 4)}...${trimmed.slice(-6)}`;
}

function firstObjectKey(value: unknown) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return "";
  return Object.keys(value as Record<string, unknown>)[0] || "";
}

function collectStringValues(value: unknown, output: string[] = []) {
  if (typeof value === "string") { output.push(value); return output; }
  if (Array.isArray(value)) { value.forEach((item) => collectStringValues(item, output)); return output; }
  if (value && typeof value === "object") { Object.values(value as Record<string, unknown>).forEach((item) => collectStringValues(item, output)); }
  return output;
}

function findFirstPattern(values: string[], pattern: RegExp) {
  for (const value of values) { const match = value.match(pattern); if (match?.[0]) return match[0]; }
  return "";
}

function extractProfileUid(values: string[]) {
  for (const value of values) { const match = value.match(/profile\.(\d+)/); if (match?.[1]) return match[1]; }
  return "";
}

function formatDouyinPasswordLabel(result: DouyinPasswordStatusResult | null) {
  if (!result) return "-";
  if (result.status === "loading") return "检测中";
  if (result.hasPassword === true) return "已设置";
  if (result.hasPassword === false) return "未设置";
  return "失败";
}

function formatDouyinCertificationLabel(result: DouyinCertificationStatusResult | null) {
  if (!result) return "-";
  if (result.status === "loading") return "检测中";
  if (result.isVerified === true) return "已实名";
  if (result.isVerified === false) return "未实名";
  return "失败";
}

function formatDouyinTokenLabel(result: DouyinTokenStatusResult | null) {
  if (!result) return "-";
  if (result.status === "loading") return "检测中";
  if (result.status === "ok") return "在线";
  if (result.status === "invalid") return "掉线";
  if (result.status === "skipped_act_token") return "跳过(act)";
  if (result.status === "request_error") return "请求失败";
  if (result.status === "http_error") return "HTTP 失败";
  if (result.status === "parse_error") return "解析失败";
  if (result.status.startsWith("missing_")) return "缺参数";
  return "失败";
}

function formatDouyinEndpointName(name: string) {
  switch (name) {
    case "safety_portrait": return "安全画像接口";
    case "profile_self": return "个人资料接口";
    default: return name || "-";
  }
}

function formatDouyinEndpointStatus(endpoint: DouyinTokenEndpointResult) {
  if (endpoint.status === "ok") return "通过";
  if (endpoint.status === "invalid") return "未通过";
  if (endpoint.status === "http_error") return "HTTP 失败";
  if (endpoint.status === "request_error") return "请求失败";
  return "解析失败";
}

function formatToutiaoCertificationLabel(result: ToutiaoCertificationStatusResult | null) {
  if (!result) return "-";
  if (result.status === "loading") return "检测中";
  if (result.isVerified === true) return "已实名";
  if (result.isVerified === false) return "未实名";
  return "失败";
}

function filterZipDropPaths(paths: string[]) {
  return paths.filter((path) => path.toLowerCase().endsWith(".zip"));
}

function uniqueZipPaths(path: string, index: number, paths: string[]) {
  return path.trim().length > 0 && paths.indexOf(path) === index;
}

function buildInitialBatchRows(apps: AppSummary[], appTypeFilter: DetectionPlatform | "all" = "all"): BatchDetectionRow[] {
  return apps
    .map<BatchDetectionRow | null>((app, index) => {
      const appType = getTrackedAppType(app.appId);
      if (!appType) return null;
      if (appTypeFilter !== "all" && appType !== appTypeFilter) return null;
      return {
        key: buildAppKey(app),
        sourceZip: app.sourceZip,
        appId: app.appId,
        appName: app.displayName,
        appType,
        aid: appType === "douyin" ? "1128" : "13",
        orderLabel: String(index + 1).padStart(3, "0"),
        fullParams: appType === "douyin" ? "待提取" : "无",
        childLockStatus: "待检测",
        accountName: "未实名",
        bindingSummary: "-",
        toutiaoBinding: "-",
        toutiaoPlatformScreenName: "-",
        qqBinding: "-",
        qqPlatformScreenName: "-",
        googleBinding: "-",
        googlePlatformScreenName: "-",
        appleIdBinding: "-",
        appleIdPlatformScreenName: "-",
        wechatBinding: "-",
        wechatPlatformScreenName: "-",
        secUid: "-",
        uid: "-",
        uniqueId: "-",
        phoneNumber: "-",
        registerTime: "-",
        awemeCount: "-",
        followingCount: "-",
        likedCount: "-",
        tokenStatus: "待检测",
        passwordStatus: "待检测",
        certificationStatus: "待检测",
        normalFunctions: "",
        limitedFunctions: "",
        durationMs: null,
        status: "pending",
        error: null,
      };
    })
    .filter((row): row is BatchDetectionRow => Boolean(row));
}

async function runBatchDetectionForRow(row: BatchDetectionRow, options: BatchDetectionOptions): Promise<BatchDetectionRow[]> {
  const normalFunctions: string[] = [];
  const limitedFunctions: string[] = [];
  const errors: string[] = [];
  let accountName = row.accountName;
  let secUid = row.secUid;
  let uid = row.uid;
  let uniqueId = row.uniqueId;
  let tokenStatus = options.token ? "未检测" : "跳过";
  let passwordStatus = row.appType === "douyin" && options.password ? "未检测" : "跳过";
  let certificationStatus = options.certification ? "未检测" : "跳过";
  let childLockStatus = row.childLockStatus;
  let phoneNumber = row.phoneNumber;
  let registerTime = row.registerTime;
  let awemeCount = row.awemeCount;
  let followingCount = row.followingCount;
  let likedCount = row.likedCount;
  let bindingSummary = row.bindingSummary;
  let toutiaoBinding = row.toutiaoBinding;
  let toutiaoPlatformScreenName = row.toutiaoPlatformScreenName;
  let qqBinding = row.qqBinding;
  let qqPlatformScreenName = row.qqPlatformScreenName;
  let googleBinding = row.googleBinding;
  let googlePlatformScreenName = row.googlePlatformScreenName;
  let appleIdBinding = row.appleIdBinding;
  let appleIdPlatformScreenName = row.appleIdPlatformScreenName;
  let wechatBinding = row.wechatBinding;
  let wechatPlatformScreenName = row.wechatPlatformScreenName;
  let fullParams = row.fullParams;
  let onlineSignal = false;
  let offlineSignal = false;
  const shouldFetchDouyinSession = row.appType === "douyin" && (options.password || options.registrationTime);

  if (row.appType === "douyin") {
    try {
      const requestParams = await invoke<DouyinRequestParamsResult>("extract_douyin_request_params", { zipPath: row.sourceZip });
      fullParams = requestParams.headerText || "-";
      secUid = requestParams.secUserId?.trim() || secUid;
    } catch (error) {
      fullParams = `提取失败：${String(error)}`;
      errors.push(fullParams);
    }

    try {
      const creds = await invoke<DouyinAccountCredentialResult>("extract_douyin_account_credentials", { zipPath: row.sourceZip });
      if (creds.accounts && creds.accounts.length > 0) {
        const uids = creds.accounts.map(a => a.uid || "-").filter(Boolean);
        const secUids = creds.accounts.map(a => a.secUid || "-").filter(Boolean);
        const uniqueIds = creds.accounts.map(a => a.uniqueId || a.shortId || "-").filter(Boolean);
        const nicknames = creds.accounts.map(a => a.nickname || "未实名").filter(Boolean);
        const currentLocalAccount = creds.accounts.find((account) => account.isCurrent) || (creds.accounts.length === 1 ? creds.accounts[0] : null);
        
        uid = dedupeText(uids).join(" | ");
        secUid = dedupeText(secUids).join(" | ");
        uniqueId = dedupeText(uniqueIds).join(" | ");
        accountName = dedupeText(nicknames).join(" | ");
        if (currentLocalAccount) {
          uid = currentLocalAccount.uid || uid;
          secUid = currentLocalAccount.secUid || secUid;
          uniqueId = currentLocalAccount.uniqueId || currentLocalAccount.shortId || uniqueId;
          accountName = currentLocalAccount.nickname || accountName;
          phoneNumber = currentLocalAccount.phoneNumber || phoneNumber;
          registerTime = formatRegisterTime(currentLocalAccount.registerTime) || registerTime;
          awemeCount = currentLocalAccount.awemeCount || awemeCount;
          followingCount = currentLocalAccount.followingCount || followingCount;
          likedCount = currentLocalAccount.likedCount || likedCount;
          bindingSummary = currentLocalAccount.bindings.summary || bindingSummary;
          toutiaoBinding = currentLocalAccount.bindings.toutiao || toutiaoBinding;
          toutiaoPlatformScreenName = currentLocalAccount.bindings.toutiaoPlatformScreenName || toutiaoPlatformScreenName;
          qqBinding = currentLocalAccount.bindings.qq || qqBinding;
          qqPlatformScreenName = currentLocalAccount.bindings.qqPlatformScreenName || qqPlatformScreenName;
          googleBinding = currentLocalAccount.bindings.google || googleBinding;
          googlePlatformScreenName = currentLocalAccount.bindings.googlePlatformScreenName || googlePlatformScreenName;
          appleIdBinding = currentLocalAccount.bindings.appleId || appleIdBinding;
          appleIdPlatformScreenName = currentLocalAccount.bindings.appleIdPlatformScreenName || appleIdPlatformScreenName;
          wechatBinding = currentLocalAccount.bindings.wechat || wechatBinding;
          wechatPlatformScreenName = currentLocalAccount.bindings.wechatPlatformScreenName || wechatPlatformScreenName;
          normalFunctions.push(...currentLocalAccount.normalFunctions);
        }
      }
    } catch (error) {
      // ignore or log
    }

    if (options.token) {
      const tokenResult = await invoke<DouyinTokenStatusResult>("check_douyin_token_status", { zipPath: row.sourceZip });
      tokenStatus = formatDouyinTokenLabel(tokenResult);
      const endpointAccount = tokenResult.endpoints.find((endpoint) => endpoint.nickname || endpoint.uid);
      accountName = endpointAccount?.nickname || endpointAccount?.uid || accountName || "未实名";
      secUid = firstEndpointValue(tokenResult.endpoints, "secUid") || secUid;
      uid = firstEndpointValue(tokenResult.endpoints, "uid") || uid;
      phoneNumber = tokenResult.localPhoneNumber || firstEndpointValue(tokenResult.endpoints, "phoneNumber") || phoneNumber;
      registerTime = formatRegisterTime(firstEndpointValue(tokenResult.endpoints, "registerTime")) || registerTime;
      awemeCount = firstEndpointValue(tokenResult.endpoints, "awemeCount") || awemeCount;
      followingCount = firstEndpointValue(tokenResult.endpoints, "followingCount") || followingCount;
      likedCount = firstEndpointValue(tokenResult.endpoints, "likedCount") || likedCount;
      if (secUid && secUid !== "-" && (!uid || uid === "-" || !uniqueId || uniqueId === "-")) {
        try {
          const identityResult = await invoke<DouyinUniqueIdResult>("resolve_douyin_unique_id", { secUid });
          secUid = identityResult.secUid || secUid;
          uid = identityResult.uid || uid;
          uniqueId = identityResult.uniqueId || uniqueId;
        } catch (error) {
          errors.push(`抖音身份补全失败：${String(error)}`);
        }
      }
      const hasFunctionItems = tokenResult.functions.length > 0;
      for (const fn of tokenResult.functions) {
        if (fn.funcAvailable) {
          normalFunctions.push(fn.funcName);
        } else {
          limitedFunctions.push(fn.funcName);
        }
      }
      if (tokenResult.status === "ok") {
        onlineSignal = true;
        childLockStatus = "无";
      } else if (tokenResult.status === "invalid") {
        offlineSignal = true;
        childLockStatus = "未知";
        if (!hasFunctionItems) {
          limitedFunctions.push("Token 失效");
        }
      } else if (tokenResult.status.startsWith("missing_")) {
        childLockStatus = "未知";
        if (!hasFunctionItems) {
          limitedFunctions.push("Token 缺参数");
        }
      } else if (tokenResult.status === "skipped_act_token") {
        childLockStatus = "未知";
      } else {
        childLockStatus = "未知";
      }
      if (tokenResult.error) errors.push(tokenResult.error);
    }

    if (shouldFetchDouyinSession) {
      const passwordResult = await invoke<DouyinPasswordStatusResult>("check_douyin_password_status", { zipPath: row.sourceZip });
      accountName = passwordResult.accountName || accountName || "未实名";
      registerTime = formatRegisterTime(passwordResult.registerTime) || registerTime;
      bindingSummary = passwordResult.bindings.summary || bindingSummary;
      toutiaoBinding = passwordResult.bindings.toutiao || toutiaoBinding;
      toutiaoPlatformScreenName = passwordResult.bindings.toutiaoPlatformScreenName || toutiaoPlatformScreenName;
      qqBinding = passwordResult.bindings.qq || qqBinding;
      qqPlatformScreenName = passwordResult.bindings.qqPlatformScreenName || qqPlatformScreenName;
      googleBinding = passwordResult.bindings.google || googleBinding;
      googlePlatformScreenName = passwordResult.bindings.googlePlatformScreenName || googlePlatformScreenName;
      appleIdBinding = passwordResult.bindings.appleId || appleIdBinding;
      appleIdPlatformScreenName = passwordResult.bindings.appleIdPlatformScreenName || appleIdPlatformScreenName;
      wechatBinding = passwordResult.bindings.wechat || wechatBinding;
      wechatPlatformScreenName = passwordResult.bindings.wechatPlatformScreenName || wechatPlatformScreenName;
      if (options.password) {
        passwordStatus = formatDouyinPasswordLabel(passwordResult);
        if (passwordResult.hasPassword === true) {
          normalFunctions.push("改密功能");
        } else if (passwordResult.hasPassword === false) {
          limitedFunctions.push("未设置密码");
        } else if (passwordResult.error) {
          errors.push(passwordResult.error);
        }
      }
    }

    if (options.certification) {
      const certificationResult = await invoke<DouyinCertificationStatusResult>("check_douyin_certification_status", { zipPath: row.sourceZip });
      certificationStatus = formatDouyinCertificationLabel(certificationResult);
      accountName = certificationResult.accountName || accountName || "未实名";
      if (certificationResult.isVerified === true) {
        normalFunctions.push("实名正常");
      } else if (certificationResult.isVerified === false) {
        limitedFunctions.push("未实名");
      } else if (certificationResult.error) {
        errors.push(certificationResult.error);
      }
    }
  } else {
    passwordStatus = "无";
    childLockStatus = "无";
    if (options.token) {
      try {
        const tokenResult = await invoke<ToutiaoTokenStatusResult>("check_toutiao_token_status", { zipPath: row.sourceZip });
        tokenStatus = formatToutiaoTokenStatus(tokenResult.status);
        accountName = tokenResult.nickname || accountName;
        uid = tokenResult.uid || uid;
        registerTime = formatRegisterTime(tokenResult.registerTime) || registerTime;
        fullParams = [
          `app_name=news_article`,
          `device_id=${tokenResult.deviceId || "-"}`,
          `aid=13`,
          `iid=${tokenResult.iid || "-"}`,
          `detail=my_tabs_v2`,
          `user_app_id=1128`,
        ].join("\n");

        if (tokenResult.status === "ok") {
          onlineSignal = true;
          normalFunctions.push("Token 在线", "登录功能");
        } else if (tokenResult.status === "invalid") {
          offlineSignal = true;
          limitedFunctions.push("Token 失效");
        } else {
          limitedFunctions.push(tokenStatus);
        }
        if (tokenResult.error) errors.push(tokenResult.error);
      } catch (error) {
        tokenStatus = "请求失败";
        limitedFunctions.push("Token 请求失败");
        errors.push(`toutiao_token_command_failed: ${String(error)}`);
      }
    }
    if (options.certification) {
      const certificationResult = await invoke<ToutiaoCertificationStatusResult>("check_toutiao_certification_status", { zipPath: row.sourceZip });
      certificationStatus = formatToutiaoCertificationLabel(certificationResult);
      if (certificationResult.isVerified === true) {
        onlineSignal = true;
        normalFunctions.push("登录功能", "实名正常");
      } else if (certificationResult.isVerified === false) {
        offlineSignal = true;
        limitedFunctions.push("未实名");
      } else if (certificationResult.error) {
        errors.push(certificationResult.error);
      }
    }
  }

  const status: BatchDetectionRow["status"] = resolveDetectionStatus({
    hasErrors: errors.length > 0,
    onlineSignal,
    offlineSignal,
  });

  const baseRow: BatchDetectionRow = {
    ...row,
    childLockStatus,
    accountName: accountName || "未实名",
    bindingSummary: bindingSummary || "-",
    toutiaoBinding: toutiaoBinding || "-",
    toutiaoPlatformScreenName: toutiaoPlatformScreenName || "-",
    qqBinding: qqBinding || "-",
    qqPlatformScreenName: qqPlatformScreenName || "-",
    googleBinding: googleBinding || "-",
    googlePlatformScreenName: googlePlatformScreenName || "-",
    appleIdBinding: appleIdBinding || "-",
    appleIdPlatformScreenName: appleIdPlatformScreenName || "-",
    wechatBinding: wechatBinding || "-",
    wechatPlatformScreenName: wechatPlatformScreenName || "-",
    secUid: secUid || "-",
    uid: uid || "-",
    uniqueId: uniqueId || "-",
    phoneNumber,
    registerTime,
    awemeCount,
    followingCount,
    likedCount,
    fullParams,
    tokenStatus,
    passwordStatus,
    certificationStatus,
    normalFunctions: dedupeText(normalFunctions).join("｜"),
    limitedFunctions: dedupeText(limitedFunctions).join("｜"),
    status,
    error: errors.join("；") || null,
  };

  if (row.appType === "douyin") {
    try {
      const creds = await invoke<DouyinAccountCredentialResult>("extract_douyin_account_credentials", { zipPath: row.sourceZip });
      if (creds.accounts && creds.accounts.length > 1) {
        const activeUid = creds.accounts.find((account) => account.isCurrent)?.uid || (baseRow.uid !== "-" && !baseRow.uid.includes(" | ") ? baseRow.uid : null);
        const mappedRows = await Promise.all(creds.accounts.map(async (acc) => {
          const isCurrent = acc.isCurrent || acc.uid === activeUid;
          const accHasActToken = isActStyleToken(acc.accessToken);
          const shouldFetchSessionDetails = options.password || options.registrationTime;
          let accTokenStatus = isCurrent ? baseRow.tokenStatus : (options.token ? "未知(非当前)" : "跳过");
          let accFullParams = isCurrent ? baseRow.fullParams : "-";
          let accPasswordStatus = isCurrent ? baseRow.passwordStatus : (options.password ? "未知(非当前)" : "跳过");
          let accAccountName = acc.nickname || (isCurrent ? baseRow.accountName : "未实名");
          let accBindingSummary = acc.bindings.summary || (isCurrent ? baseRow.bindingSummary : "-");
          let accToutiaoBinding = acc.bindings.toutiao || (isCurrent ? baseRow.toutiaoBinding : "-");
          let accToutiaoPlatformScreenName = acc.bindings.toutiaoPlatformScreenName || (isCurrent ? baseRow.toutiaoPlatformScreenName : "-");
          let accQqBinding = acc.bindings.qq || (isCurrent ? baseRow.qqBinding : "-");
          let accQqPlatformScreenName = acc.bindings.qqPlatformScreenName || (isCurrent ? baseRow.qqPlatformScreenName : "-");
          let accGoogleBinding = acc.bindings.google || (isCurrent ? baseRow.googleBinding : "-");
          let accGooglePlatformScreenName = acc.bindings.googlePlatformScreenName || (isCurrent ? baseRow.googlePlatformScreenName : "-");
          let accAppleIdBinding = acc.bindings.appleId || (isCurrent ? baseRow.appleIdBinding : "-");
          let accAppleIdPlatformScreenName = acc.bindings.appleIdPlatformScreenName || (isCurrent ? baseRow.appleIdPlatformScreenName : "-");
          let accWechatBinding = acc.bindings.wechat || (isCurrent ? baseRow.wechatBinding : "-");
          let accWechatPlatformScreenName = acc.bindings.wechatPlatformScreenName || (isCurrent ? baseRow.wechatPlatformScreenName : "-");
          let accUid = acc.uid || "-";
          let accSecUid = acc.secUid || "-";
          let accUniqueId = acc.uniqueId || acc.shortId || "-";
          let accPhoneNumber = acc.phoneNumber || (isCurrent ? baseRow.phoneNumber : "-");
          let accRegisterTime = formatRegisterTime(acc.registerTime) || (isCurrent ? baseRow.registerTime : "-");
          let accAwemeCount = acc.awemeCount || (isCurrent ? baseRow.awemeCount : "-");
          let accFollowingCount = acc.followingCount || (isCurrent ? baseRow.followingCount : "-");
          let accLikedCount = acc.likedCount || (isCurrent ? baseRow.likedCount : "-");
          const accNormalFunctions = isCurrent ? splitDisplayText(baseRow.normalFunctions) : [...acc.normalFunctions];
          const accLimitedFunctions = isCurrent ? splitDisplayText(baseRow.limitedFunctions) : [];
          
          if (!isCurrent) {
            if (!acc.accessToken) {
              accFullParams = "-";
            } else if (accHasActToken) {
              accFullParams = "跳过(act token)";
              if (options.token) {
                accTokenStatus = "跳过(act token)";
              }
            } else {
              try {
                const requestParams = await invoke<DouyinRequestParamsResult>("extract_douyin_request_params", {
                  zipPath: row.sourceZip,
                  tokenOverride: acc.accessToken
                });
                accFullParams = requestParams.headerText || "-";
                if (requestParams.secUserId) {
                  accSecUid = requestParams.secUserId.trim();
                }
              } catch (error) {
                accFullParams = `提取失败：${String(error)}`;
              }
              if (options.token && acc.accessToken) {
                try {
                  const tr = await invoke<DouyinTokenStatusResult>("check_douyin_token_status", {
                    zipPath: row.sourceZip,
                    tokenOverride: acc.accessToken
                  });
                  accTokenStatus = formatDouyinTokenLabel(tr);
                  accPhoneNumber = tr.localPhoneNumber || accPhoneNumber;
                  for (const fn of tr.functions) {
                    if (fn.funcAvailable) {
                      accNormalFunctions.push(fn.funcName);
                    } else {
                      accLimitedFunctions.push(fn.funcName);
                    }
                  }
                  if (tr.validEndpointCount > 0) {
                    const validEndpoint = tr.endpoints.find(e => e.status === "ok");
                    if (validEndpoint) {
                      if (validEndpoint.uid) accUid = validEndpoint.uid;
                      if (validEndpoint.secUid) accSecUid = validEndpoint.secUid;
                      if (validEndpoint.nickname) accAccountName = validEndpoint.nickname;
                      if (validEndpoint.phoneNumber) accPhoneNumber = validEndpoint.phoneNumber;
                      if (validEndpoint.registerTime) accRegisterTime = formatRegisterTime(validEndpoint.registerTime) || accRegisterTime;
                      if (validEndpoint.awemeCount !== null) accAwemeCount = String(validEndpoint.awemeCount);
                      if (validEndpoint.followingCount !== null) accFollowingCount = String(validEndpoint.followingCount);
                      if (validEndpoint.likedCount !== null) accLikedCount = String(validEndpoint.likedCount);
                    }
                  }
                  if (tr.status === "invalid" && tr.functions.length === 0) {
                    accLimitedFunctions.push("Token 失效");
                  } else if (tr.status.startsWith("missing_") && tr.functions.length === 0) {
                    accLimitedFunctions.push("Token 缺参数");
                  }
                } catch (e) {
                  // ignore
                }
              }
            }
          }

          if (!isCurrent && acc.sessionId && shouldFetchSessionDetails) {
            try {
              const pr = await invoke<DouyinPasswordStatusResult>("check_douyin_password_status", { 
                zipPath: row.sourceZip, 
                sessionIdOverride: acc.sessionId 
              });
              if (options.password) {
                accPasswordStatus = formatDouyinPasswordLabel(pr);
                if (pr.hasPassword === true) {
                  accNormalFunctions.push("改密功能");
                } else if (pr.hasPassword === false) {
                  accLimitedFunctions.push("未设置密码");
                }
              }
              if (pr.accountName) accAccountName = pr.accountName;
              if (pr.registerTime) accRegisterTime = formatRegisterTime(pr.registerTime) || accRegisterTime;
              if (pr.bindings.summary) accBindingSummary = pr.bindings.summary;
              if (pr.bindings.toutiao) accToutiaoBinding = pr.bindings.toutiao;
              if (pr.bindings.toutiaoPlatformScreenName) accToutiaoPlatformScreenName = pr.bindings.toutiaoPlatformScreenName;
              if (pr.bindings.qq) accQqBinding = pr.bindings.qq;
              if (pr.bindings.qqPlatformScreenName) accQqPlatformScreenName = pr.bindings.qqPlatformScreenName;
              if (pr.bindings.google) accGoogleBinding = pr.bindings.google;
              if (pr.bindings.googlePlatformScreenName) accGooglePlatformScreenName = pr.bindings.googlePlatformScreenName;
              if (pr.bindings.appleId) accAppleIdBinding = pr.bindings.appleId;
              if (pr.bindings.appleIdPlatformScreenName) accAppleIdPlatformScreenName = pr.bindings.appleIdPlatformScreenName;
              if (pr.bindings.wechat) accWechatBinding = pr.bindings.wechat;
              if (pr.bindings.wechatPlatformScreenName) accWechatPlatformScreenName = pr.bindings.wechatPlatformScreenName;
            } catch (e) {
              // ignore
            }
          }

          return {
            ...baseRow,
            fullParams: accFullParams,
            accountName: accAccountName,
            bindingSummary: accBindingSummary,
            toutiaoBinding: accToutiaoBinding,
            toutiaoPlatformScreenName: accToutiaoPlatformScreenName,
            qqBinding: accQqBinding,
            qqPlatformScreenName: accQqPlatformScreenName,
            googleBinding: accGoogleBinding,
            googlePlatformScreenName: accGooglePlatformScreenName,
            appleIdBinding: accAppleIdBinding,
            appleIdPlatformScreenName: accAppleIdPlatformScreenName,
            wechatBinding: accWechatBinding,
            wechatPlatformScreenName: accWechatPlatformScreenName,
            uid: accUid,
            secUid: accSecUid,
            uniqueId: accUniqueId,
            phoneNumber: accPhoneNumber,
            registerTime: accRegisterTime,
            awemeCount: accAwemeCount,
            followingCount: accFollowingCount,
            likedCount: accLikedCount,
            tokenStatus: accTokenStatus,
            passwordStatus: accPasswordStatus,
            certificationStatus: isCurrent ? baseRow.certificationStatus : "未知(非当前)",
            normalFunctions: dedupeText(accNormalFunctions).join("｜"),
            limitedFunctions: dedupeText(accLimitedFunctions).join("｜"),
            status: (isCurrent ? baseRow.status : "skipped") as BatchDetectionRow["status"],
          } as BatchDetectionRow;
        }));
        return mappedRows;
      }
    } catch (error) {
      // ignore
    }
  }

  return [baseRow];
}

function firstEndpointValue(
  endpoints: DouyinTokenEndpointResult[],
  key: keyof Pick<DouyinTokenEndpointResult, "uid" | "secUid" | "phoneNumber" | "registerTime" | "awemeCount" | "followingCount" | "likedCount">,
) {
  return endpoints.map((endpoint) => endpoint[key]).find((value): value is string => typeof value === "string" && value.trim().length > 0) ?? "";
}

function isActStyleToken(value?: string | null) {
  return typeof value === "string" && value.trim().toLowerCase().startsWith("act");
}

function formatRegisterTime(value?: string | null) {
  if (!value) return "";
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue) || numberValue <= 0) return value;
  const milliseconds = numberValue > 10_000_000_000 ? numberValue : numberValue * 1000;
  return new Date(milliseconds).toLocaleString("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}

function dedupeText(values: string[]) {
  return Array.from(new Set(values.filter(Boolean)));
}

function splitDisplayText(value?: string | null) {
  if (!value) return [];
  return value.split("｜").map((item) => item.trim()).filter(Boolean);
}

function escapeCsvCell(value: unknown) {
  const text = String(value ?? "")
    .replace(/\r\n/g, "; ")
    .replace(/[\r\n]+/g, "; ")
    .replace(/\s{2,}/g, " ")
    .replace(/(?:;\s*){2,}/g, "; ")
    .trim();
  if (/[",\n]/.test(text)) return '"' + text.replace(/"/g, '""') + '"';
  return text;
}

function firstString(row: Record<string, unknown>, keys: string[]) {
  for (const key of keys) {
    const value = row[key];
    if (typeof value === "string" && value.trim()) return value;
    if (typeof value === "number") return String(value);
  }
  return "";
}
