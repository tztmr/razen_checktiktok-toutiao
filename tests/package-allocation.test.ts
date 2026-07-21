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
