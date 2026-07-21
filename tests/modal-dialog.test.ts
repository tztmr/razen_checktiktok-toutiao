import assert from "node:assert/strict";
import test from "node:test";
import {
  getWrappedFocusIndex,
  resolveDialogOpener,
  scheduleFocusRestoration,
} from "../src/use-modal-dialog.ts";

test("wraps forward focus to the first dialog control", () => {
  assert.equal(getWrappedFocusIndex(3, 4, false), 0);
});

test("wraps backward focus to the last dialog control", () => {
  assert.equal(getWrappedFocusIndex(0, 4, true), 3);
});

test("keeps forward focus moving inside the dialog", () => {
  assert.equal(getWrappedFocusIndex(1, 4, false), 2);
});

test("keeps backward focus moving inside the dialog", () => {
  assert.equal(getWrappedFocusIndex(2, 4, true), 1);
});

test("returns no focus target for an empty dialog", () => {
  assert.equal(getWrappedFocusIndex(0, 0, false), -1);
});

test("restores opener focus after the dialog unmount frame", () => {
  const calls: string[] = [];
  const target = {
    isConnected: true,
    focus: () => calls.push("focus"),
  } as unknown as HTMLElement;

  scheduleFocusRestoration(target, (callback) => {
    calls.push("schedule");
    callback(0);
    return 1;
  });

  assert.deepEqual(calls, ["schedule", "focus"]);
});

test("prefers the explicitly clicked control over document activeElement", () => {
  const clicked = { id: "clicked" } as unknown as HTMLElement;
  const active = { id: "active", focus: () => undefined } as unknown as HTMLElement;

  assert.equal(resolveDialogOpener(clicked, active), clicked);
  assert.equal(resolveDialogOpener(null, active), active);
});
