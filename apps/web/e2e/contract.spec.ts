import { expect, test } from "@playwright/test";

test("OpenAPI document and runtime envelope stay aligned", async ({ request }) => {
  const specResponse = await request.get("/api/v1/openapi.json");
  expect(specResponse.status()).toBe(200);
  const spec = await specResponse.json();
  expect(spec.openapi).toBe("3.1.0");
  expect(spec.paths["/api/v1/instruments/{symbol}/fundamentals"]).toBeTruthy();

  const healthResponse = await request.get("/api/v1/health");
  expect(healthResponse.status()).toBe(200);
  const health = await healthResponse.json();
  expect(health.code).toBe(0);
  expect(health.request_id).toBeTruthy();
  expect(health.data.status).toBe("ok");

  const overviewResponse = await request.get("/api/v1/market/overview");
  const overview = await overviewResponse.json();
  expect(overview.code).toBe(0);
  expect(overview.data.is_demo).toBe(false);
  expect(overview.data.source).not.toContain("demo");
  if (overview.data.source === "pending") {
    expect(overview.data.as_of).toBe("");
    expect(overview.data.indices).toEqual([]);
  } else {
    expect(overview.data.as_of).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  }
  expect(Array.isArray(overview.data.indices)).toBe(true);
  expect(typeof overview.data.breadth.advance).toBe("number");

  const missing = await request.get("/api/v1/instruments/999999.SH");
  expect(missing.status()).toBe(404);
  const error = await missing.json();
  expect(error.code).toBe(1);
  expect(error.message).toContain("NOT_FOUND");
  expect(error.data).toBeNull();
});
