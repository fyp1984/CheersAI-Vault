import { expect, test } from "@playwright/test";

import { ensureRuntimeReady } from "./helpers/runtime";

test.describe("文件脱敏主流程", () => {
  test.beforeEach(async ({ request }) => {
    await ensureRuntimeReady(request);
  });

  test("用户可上传文件、生成预览并确认正式批次", async ({ page }) => {
    await page.goto("/#/process");

    const generateButton = page.getByRole("button", { name: /生成脱敏预览/ });
    await expect(generateButton).toBeDisabled();

    await page.getByLabel("选择需要脱敏的文件").setInputFiles({
      name: "playwright-flow.txt",
      mimeType: "text/plain",
      buffer: Buffer.from(
        "Name: Alice\nPhone: 13900000000\nEmail: demo@example.com\n",
        "utf8"
      ),
    });

    await expect(page.getByText("文件队列（1")).toBeVisible();
    await expect(generateButton).toBeEnabled();
    await generateButton.click();

    await expect(page.getByRole("heading", { name: "脱敏预览" })).toBeVisible();
    await expect(page.getByText("预览已就绪")).toBeVisible({ timeout: 30_000 });

    await page.getByRole("button", { name: /playwright-flow\.txt/ }).click();
    await expect(page.getByText(/\*\*\*PHONE\*\*\*/)).toBeVisible();
    await expect(page.getByText(/\*\*\*EMAIL\*\*\*/)).toBeVisible();

    await page.getByRole("button", { name: "确认并生成正式批次" }).click();
    await expect(page.getByText("批次详情")).toBeVisible({ timeout: 30_000 });
    await expect(page.getByText("批次状态")).toBeVisible({ timeout: 30_000 });
    await expect(page.getByText("已完成").first()).toBeVisible({ timeout: 30_000 });
    await expect(page.getByRole("button", { name: "下载 Markdown" })).toBeVisible();
  });
});
