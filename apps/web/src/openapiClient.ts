import createClient from "openapi-fetch";
import type { paths } from "./api.generated";
import type { MarketOverview } from "./types";

export const openapiClient = createClient<paths>({ baseUrl: "" });

export async function generatedOverview(): Promise<MarketOverview> {
  const { data, error } = await openapiClient.GET("/api/v1/market/overview");
  if (error || !data) throw new Error("market overview request failed");
  const envelope = data as { code: number; message: string; data: MarketOverview | null };
  if (envelope.code !== 0 || !envelope.data) throw new Error(envelope.message || "market overview failed");
  return envelope.data;
}
