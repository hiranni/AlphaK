import { generatedOverview } from "./openapiClient";
import type {
  BarsResponse, DataJob, Fundamentals, Health, IndexDetail, MarketOverview, NewsFeed, Profile, QualityReport,
  SearchResult, SectorDetail, SectorRow, Settings, SourceStatus, StockDetail, WatchlistGroup,
} from "./types";

const BASE = "/api/v1";

type Envelope<T> = { code: number; message: string; data: T | null; request_id: string };

export class ApiError extends Error {
  status: number;
  code: string;
  constructor(message: string, status: number, code: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${BASE}${path}`, {
    headers: { "Content-Type": "application/json", ...(init?.headers ?? {}) },
    ...init,
  });
  let body: Envelope<T> | null = null;
  try {
    body = (await response.json()) as Envelope<T>;
  } catch {
    throw new ApiError("服务返回了无法解析的响应", response.status, "BAD_RESPONSE");
  }
  if (!response.ok || body.code !== 0 || body.data === null) {
    throw new ApiError(body.message || "请求失败", response.status, body.message?.split(":")[0] || "ERROR");
  }
  return body.data;
}

export const api = {
  marketBreadth: () => request<{ data: { advance: number; decline: number; flat: number; suspended: number; limit_up: number; limit_down: number; as_of: string; source: string; scope: string } | null; stale: boolean; error: string }>("/market/breadth"),
  popularity: () => request<{data: {symbol:string;name:string;rank:number;close:number|null;change_pct:number|null}[];source:string;updated_at:number;stale:boolean}>("/market/popularity"),
  news: () => request<NewsFeed>("/market/news"),
  retrySupplements: (symbol: string) => request<{ reset: boolean }>(`/instruments/${encodeURIComponent(symbol)}/supplement/retry`, { method: "POST" }),
  supplement: <T>(symbol: string, dataset: string) => request<{ data: T; source: string; updated_at: number; stale: boolean; error: string; retry_count: number }>(`/instruments/${encodeURIComponent(symbol)}/supplement/${dataset}`),
  health: () => request<Health>("/health"),
  overview: generatedOverview,
  refreshOverview: () => request<MarketOverview>("/market/overview?refresh=true"),
  search: (q: string) => request<SearchResult[]>(`/instruments/search?q=${encodeURIComponent(q)}`),
  stock: (symbol: string) => request<StockDetail>(`/instruments/${encodeURIComponent(symbol)}`),
  bars: (symbol: string, period: string, adjust: string) =>
    request<BarsResponse>(`/instruments/${encodeURIComponent(symbol)}/bars?period=${period}&adjust=${adjust}`),
  fundamentals: (symbol: string) => request<Fundamentals>(`/instruments/${encodeURIComponent(symbol)}/fundamentals`),
  profile: (symbol: string) => request<Profile>(`/instruments/${encodeURIComponent(symbol)}/profile`),
  dailyBasic: (symbol: string) => request<import("./types").DailyBasic>(`/instruments/${encodeURIComponent(symbol)}/daily-basic`),
  index: (code: string) => request<IndexDetail>(`/indices/${encodeURIComponent(code)}`),
  sectors: () => request<SectorRow[]>("/sectors"),
  sector: (id: number) => request<SectorDetail>(`/sectors/${id}`),
  watchWorkbench: () => request<import("./types").WatchWorkbench>("/watchlists/workbench"),
  watchAlerts: () => request<import("./types").WatchAlerts>("/watchlists/alerts"),
  createWatchAlert: (body: import("./types").WatchAlertInput) => request<import("./types").WatchAlerts>("/watchlists/alerts", { method: "POST", body: JSON.stringify(body) }),
  updateWatchAlert: (id: string, body: import("./types").WatchAlertInput) => request<import("./types").WatchAlerts>(`/watchlists/alerts/${encodeURIComponent(id)}`, { method: "PUT", body: JSON.stringify(body) }),
  deleteWatchAlert: (id: string) => request<import("./types").WatchAlerts>(`/watchlists/alerts/${encodeURIComponent(id)}`, { method: "DELETE" }),
  watchlists: () => request<WatchlistGroup[]>("/watchlists"),
  refreshWatchlists: () => request<WatchlistGroup[]>("/watchlists/refresh", { method: "POST" }),
  createGroup: (name: string) => request<WatchlistGroup[]>("/watchlists/groups", { method: "POST", body: JSON.stringify({ name }) }),
  renameGroup: (id: string, name: string) => request<WatchlistGroup[]>(`/watchlists/groups/${id}`, { method: "PUT", body: JSON.stringify({ name }) }),
  updateGroup: (id: string, body: { name?: string; color?: string }) => request<WatchlistGroup[]>(`/watchlists/groups/${id}`, { method: "PUT", body: JSON.stringify(body) }),
  clearGroup: (id: string) => request<WatchlistGroup[]>(`/watchlists/groups/${id}/items`, { method: "DELETE" }),
  deleteGroup: (id: string) => request<WatchlistGroup[]>(`/watchlists/groups/${id}`, { method: "DELETE" }),
  reorderItems: (group_id: string, ids: string[]) => request<WatchlistGroup[]>("/watchlists/items/reorder", { method: "PUT", body: JSON.stringify({ group_id, ids }) }),
  addItem: (group_id: string, symbol: string, note = "") => request<WatchlistGroup[]>("/watchlists/items", { method: "POST", body: JSON.stringify({ group_id, symbol, note }) }),
  updateItem: (id: string, body: { note?: string; sort?: number }) => request<WatchlistGroup[]>(`/watchlists/items/${id}`, { method: "PUT", body: JSON.stringify(body) }),
  deleteItem: (id: string) => request<WatchlistGroup[]>(`/watchlists/items/${id}`, { method: "DELETE" }),
  settings: () => request<Settings>("/settings"),
  updateSettings: (body: Partial<Settings>) => request<Settings>("/settings", { method: "PUT", body: JSON.stringify(body) }),
  jobs: () => request<DataJob[]>("/admin/data-jobs"),
  runJob: (dataset: string) => request<DataJob[]>("/admin/data-jobs", { method: "POST", body: JSON.stringify({ dataset }) }),
  quality: () => request<QualityReport[]>("/admin/quality"),
  sourceStatus: () => request<SourceStatus>("/admin/sources"),
  sourceSync: (dataset: string, symbols: string[], start: string, end: string) =>
    request<unknown>("/admin/sources/sync", { method: "POST", body: JSON.stringify({ dataset, symbols, start, end, request_id: `ui-${Date.now()}` }) }),
};
