import assert from "node:assert/strict";
import test from "node:test";
import {
  buildBatchDetectionOptions,
  queueBatchOptionFromEvent,
  type BatchDetectionOptions,
  type DouyinDetectionOptions,
  type ToutiaoDetectionOptions,
} from "../src/batch-options.ts";

const initialOptions: BatchDetectionOptions = {
  appType: "douyin",
  token: true,
  password: true,
  certification: true,
  aid: true,
  registrationTime: true,
};

test("captures a checkbox value before React evaluates the queued updater", () => {
  let queuedUpdater: ((current: BatchDetectionOptions) => BatchDetectionOptions) | undefined;
  const event: { currentTarget: { checked: boolean } | null } = {
    currentTarget: { checked: false },
  };

  queueBatchOptionFromEvent(
    (updater) => { queuedUpdater = updater; },
    "token",
    event as { currentTarget: { checked: boolean } },
    (target) => target.checked,
  );

  event.currentTarget = null;

  assert.ok(queuedUpdater);
  assert.deepEqual(queuedUpdater(initialOptions), {
    ...initialOptions,
    token: false,
  });
});

test("captures a select value before React evaluates the queued updater", () => {
  let queuedUpdater: ((current: BatchDetectionOptions) => BatchDetectionOptions) | undefined;
  const event: { currentTarget: { value: BatchDetectionOptions["appType"] } | null } = {
    currentTarget: { value: "toutiao" },
  };

  queueBatchOptionFromEvent(
    (updater) => { queuedUpdater = updater; },
    "appType",
    event as { currentTarget: { value: BatchDetectionOptions["appType"] } },
    (target) => target.value,
  );

  event.currentTarget = null;

  assert.ok(queuedUpdater);
  assert.equal(queuedUpdater(initialOptions).appType, "toutiao");
});

const douyinOptions: DouyinDetectionOptions = {
  token: true,
  password: false,
  certification: true,
  aid: true,
  registrationTime: false,
};

const toutiaoOptions: ToutiaoDetectionOptions = {
  token: true,
  certification: true,
};

test("builds a Douyin run from Douyin controls", () => {
  assert.deepEqual(
    buildBatchDetectionOptions("douyin", douyinOptions, toutiaoOptions),
    {
      appType: "douyin",
      ...douyinOptions,
    },
  );
});

test("builds a Toutiao run without Douyin-only checks", () => {
  assert.deepEqual(
    buildBatchDetectionOptions("toutiao", douyinOptions, toutiaoOptions),
    {
      appType: "toutiao",
      token: true,
      password: false,
      certification: true,
      aid: false,
      registrationTime: false,
    },
  );
});

test("keeps the Toutiao token option independent from Douyin", () => {
  assert.deepEqual(
    buildBatchDetectionOptions(
      "toutiao",
      { ...douyinOptions, token: true },
      { ...toutiaoOptions, token: false },
    ),
    {
      appType: "toutiao",
      token: false,
      password: false,
      certification: true,
      aid: false,
      registrationTime: false,
    },
  );
});
