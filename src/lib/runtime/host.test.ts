import test from "node:test";
import assert from "node:assert/strict";

import { detectHost, isBrowserHost, isTauriHost } from "./host";

function withTauriFlag(value: boolean | undefined, run: () => void) {
  const original = (globalThis as { isTauri?: boolean }).isTauri;
  if (value === undefined) {
    delete (globalThis as { isTauri?: boolean }).isTauri;
  } else {
    (globalThis as { isTauri?: boolean }).isTauri = value;
  }

  try {
    run();
  } finally {
    if (original === undefined) {
      delete (globalThis as { isTauri?: boolean }).isTauri;
    } else {
      (globalThis as { isTauri?: boolean }).isTauri = original;
    }
  }
}

test("detectHost returns browser when Tauri flag is absent", () => {
  withTauriFlag(undefined, () => {
    assert.equal(detectHost(), "browser");
    assert.equal(isBrowserHost(), true);
    assert.equal(isTauriHost(), false);
  });
});

test("detectHost returns tauri when Tauri flag is present", () => {
  withTauriFlag(true, () => {
    assert.equal(detectHost(), "tauri");
    assert.equal(isBrowserHost(), false);
    assert.equal(isTauriHost(), true);
  });
});

test("detectHost treats falsy Tauri flag as browser", () => {
  withTauriFlag(false, () => {
    assert.equal(detectHost(), "browser");
    assert.equal(isBrowserHost(), true);
    assert.equal(isTauriHost(), false);
  });
});
