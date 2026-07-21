import assert from "node:assert/strict";
import test from "node:test";
import {
  formatDetectionDuration,
  formatToutiaoTokenStatus,
  getDetectionStatusLabel,
  resolveDetectionStatus,
} from "../src/workbench-ui.ts";

test("formats short durations in milliseconds", () => {
  assert.equal(formatDetectionDuration(842), "842 ms");
});

test("formats long durations in seconds", () => {
  assert.equal(formatDetectionDuration(1420), "1.42 s");
});

test("formats an empty duration as a dash", () => {
  assert.equal(formatDetectionDuration(null), "-");
});

test("formats detection statuses in Chinese", () => {
  assert.equal(getDetectionStatusLabel("pending"), "待检测");
  assert.equal(getDetectionStatusLabel("checking"), "检测中");
  assert.equal(getDetectionStatusLabel("online"), "在线");
  assert.equal(getDetectionStatusLabel("offline"), "掉线");
  assert.equal(getDetectionStatusLabel("failed"), "失败");
  assert.equal(getDetectionStatusLabel("skipped"), "已跳过");
});

test("formats Toutiao token outcomes without conflating failures", () => {
  assert.equal(formatToutiaoTokenStatus("ok"), "在线");
  assert.equal(formatToutiaoTokenStatus("invalid"), "掉线");
  assert.equal(formatToutiaoTokenStatus("missing_iid"), "缺参数");
  assert.equal(formatToutiaoTokenStatus("http_error"), "HTTP 失败");
  assert.equal(formatToutiaoTokenStatus("request_error"), "请求失败");
  assert.equal(formatToutiaoTokenStatus("parse_error"), "解析失败");
});

test("keeps a confirmed logged-out account offline when another check has an error", () => {
  assert.equal(resolveDetectionStatus({
    hasErrors: true,
    onlineSignal: false,
    offlineSignal: true,
  }), "offline");
});
