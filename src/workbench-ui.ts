export type DetectionStatus =
  | "pending"
  | "checking"
  | "online"
  | "offline"
  | "failed"
  | "skipped";

const DETECTION_STATUS_LABELS: Record<DetectionStatus, string> = {
  pending: "待检测",
  checking: "检测中",
  online: "在线",
  offline: "掉线",
  failed: "失败",
  skipped: "已跳过",
};

export function formatDetectionDuration(durationMs: number | null) {
  if (durationMs == null) return "-";
  if (durationMs < 1000) return `${Math.round(durationMs)} ms`;
  return `${(durationMs / 1000).toFixed(2).replace(/\.00$/, "")} s`;
}

export function getDetectionStatusLabel(status: DetectionStatus) {
  return DETECTION_STATUS_LABELS[status];
}

export function formatToutiaoTokenStatus(status: string) {
  if (status === "ok") return "在线";
  if (status === "invalid") return "掉线";
  if (status.startsWith("missing_")) return "缺参数";
  if (status === "http_error") return "HTTP 失败";
  if (status === "request_error") return "请求失败";
  if (status === "parse_error") return "解析失败";
  if (status === "loading") return "检测中";
  return "失败";
}

export function resolveDetectionStatus({
  hasErrors,
  onlineSignal,
  offlineSignal,
}: {
  hasErrors: boolean;
  onlineSignal: boolean;
  offlineSignal: boolean;
}): DetectionStatus {
  if (onlineSignal) return "online";
  if (offlineSignal) return "offline";
  if (hasErrors) return "failed";
  return "skipped";
}
