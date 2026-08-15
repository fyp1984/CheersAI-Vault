import { expect, type APIRequestContext } from "@playwright/test";

export interface RuntimeBatchFileSummary {
  file_id: string;
  display_name: string;
  status: string;
  artifact_id: string | null;
  restore_available?: boolean;
}

export interface CompletedBatch {
  batchId: string;
  files: RuntimeBatchFileSummary[];
}

export async function ensureRuntimeReady(request: APIRequestContext) {
  const response = await request.get("/api/v1/health");
  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as { status?: string };
  expect(payload.status).toBe("ready");
}

export async function createCompletedBatch(
  request: APIRequestContext,
  options?: {
    fileName?: string;
    content?: string;
    ruleIds?: string[];
  }
): Promise<CompletedBatch> {
  const fileName = options?.fileName ?? `playwright-${Date.now()}.txt`;
  const content =
    options?.content ??
    "Name: Alice\nPhone: 13900000000\nEmail: demo@example.com\n";
  const ruleIds = options?.ruleIds ?? ["phone", "email"];

  const createResponse = await request.post("/api/v1/batches", {
    multipart: {
      files: {
        name: fileName,
        mimeType: "text/plain",
        buffer: Buffer.from(content, "utf8"),
      },
      rule_ids: JSON.stringify(ruleIds),
    },
    timeout: 60_000,
  });

  expect(createResponse.ok()).toBeTruthy();
  const created = (await createResponse.json()) as { batch_id: string };
  expect(created.batch_id).toBeTruthy();

  for (let attempt = 0; attempt < 40; attempt += 1) {
    const detailResponse = await request.get(`/api/v1/batches/${created.batch_id}`, {
      timeout: 30_000,
    });
    expect(detailResponse.ok()).toBeTruthy();

    const detail = (await detailResponse.json()) as {
      batch: { status: string };
      files: RuntimeBatchFileSummary[];
    };

    if (["Completed", "CompletedWithErrors", "Failed"].includes(detail.batch.status)) {
      expect(detail.batch.status).toBe("Completed");
      return {
        batchId: created.batch_id,
        files: detail.files,
      };
    }

    await new Promise((resolve) => setTimeout(resolve, 1_000));
  }

  throw new Error(`批次 ${created.batch_id} 未在预期时间内完成`);
}
