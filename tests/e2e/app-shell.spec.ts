import { expect, test } from "@playwright/test";

import { ensureRuntimeReady } from "./helpers/runtime";

test.describe("客户端主导航与兼容性基线", () => {
  test.beforeEach(async ({ request }) => {
    await ensureRuntimeReady(request);
  });

  test("浏览器宿主可访问核心页面并展示 Runtime 在线状态", async ({ page }) => {
    await page.goto("/#/process");

    const sidebar = page.locator("aside");
    await expect(sidebar.getByText("Runtime 状态")).toBeVisible();
    await expect(sidebar.getByText("已连接")).toBeVisible();
    await expect(page.getByRole("button", { name: /生成脱敏预览/ })).toBeVisible();

    await page.getByRole("link", { name: "规则配置" }).click();
    await expect(page.getByText("敏感词列表")).toBeVisible();

    await page.getByRole("link", { name: "增强服务" }).click();
    await expect(page.getByRole("heading", { name: "增强服务" })).toBeVisible();
    await expect(page.getByText("OCR 文字识别服务")).toBeVisible();

    await page.getByRole("link", { name: "文件管理" }).click();
    await expect(page.locator("main header").getByText("文件管理")).toBeVisible();

    await page.getByRole("link", { name: "文件反脱敏" }).click();
    await expect(page.locator("main header").getByText("文件反脱敏")).toBeVisible();

    await page.getByRole("link", { name: "操作日志" }).click();
    await expect(page.locator("main header").getByText("操作日志")).toBeVisible();
  });
});
