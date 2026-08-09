import { expect, test } from "@playwright/test";

import { createCompletedBatch, ensureRuntimeReady } from "./helpers/runtime";

test.describe("批次管理与反脱敏流程", () => {
  test.beforeEach(async ({ request }) => {
    await ensureRuntimeReady(request);
  });

  test("文件管理页可读取已完成批次并展示下载入口", async ({ page, request }) => {
    const batch = await createCompletedBatch(request, {
      fileName: "playwright-manager.txt",
    });

    await page.goto(`/#/files?batch=${batch.batchId}`);

    await expect(page.getByText("批次状态")).toBeVisible();
    await expect(page.getByText("playwright-manager.txt")).toBeVisible();
    await expect(page.getByRole("button", { name: "下载 Markdown" })).toBeVisible();
  });

  test("反脱敏页面可选择可恢复文件并触发下载", async ({ page, request }) => {
    const batch = await createCompletedBatch(request, {
      fileName: "playwright-restore.txt",
    });
    const restorable = batch.files.find((file) => file.artifact_id);

    expect(restorable?.artifact_id).toBeTruthy();

    await page.goto(`/#/unmask?batch_id=${batch.batchId}`);
    await expect(page.getByText("选择一个可恢复的文件")).toBeVisible();
    await page.getByRole("button", { name: "选择" }).first().click();

    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "开始反脱敏" }).click();

    await expect(page.getByText("反脱敏成功")).toBeVisible({ timeout: 30_000 });
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toBeTruthy();
  });
});
