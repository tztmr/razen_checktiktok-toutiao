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
