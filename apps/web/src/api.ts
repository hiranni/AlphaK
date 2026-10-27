import type { ResearchConfig, ResearchWorkspace } from "./components/TradingResearch";
import type { QuantCatalog, QuantObservation } from "./components/QuantStrategies";
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

export type PaperStrategy = {id:string;archived:boolean;workspace_id:string;initial_cash_fen:number;cash_fen:number;config:ResearchConfig;policy:{buy_lot:number;sell_rule:string;limit_rule:string;sessions:string};created_at:string};

export type ExecutionRecords = {
 execution_ready:boolean; rules_version:string; cash_fen:number;
 orders:{id:string;run_id:string;symbol:string;name:string;side:string;quantity:number;price_fen:number|null;fee_fen:number|null;status:string;reason:string;created_at:string}[];
 runs:{id:string;analysis_id:string|null;rules_version:string;created_at:string}[];
 positions:{symbol:string;name:string;quantity:number;available:number;cost_fen:number}[];
 nav:{created_at:string;cash_fen:number;market_fen:number|null;equity_fen:number|null}[];
 cash_ledger:{amount_fen:number;created_at:string;symbol:string;side:string}[];
};

export const api = {
  executionRecords:(id:string)=>request<ExecutionRecords>(`/trading/paper-strategies/${encodeURIComponent(id)}/execution`),
  checkExecution:(id:string,request_key:string)=>request<{id:string;status:string}>(`/trading/paper-strategies/${encodeURIComponent(id)}/execution`,{method:'POST',body:JSON.stringify({request_key})}),
  createPaperAccount: (body:{request_key:string;config:ResearchConfig;initial_cash_fen:number}) => request<{id:string}>("/trading/paper-strategies",{method:"POST",body:JSON.stringify(body)}),
  updatePaperAccount: (id:string,body:{action:'archive'|'restore'|'reconfigure';config?:ResearchConfig}) => request<unknown>(`/trading/paper-strategies/${encodeURIComponent(id)}`,{method:"PATCH",body:JSON.stringify(body)}),
  paperAccountEvents: (id:string) => request<{kind:string;payload:{previous_workspace_id?:string;workspace_id?:string};created_at:string}[]>(`/trading/paper-strategies/${encodeURIComponent(id)}/events`),
  paperStrategies: () => request<PaperStrategy[]>("/trading/paper-strategies"),
  researchWorkspaces: () => request<ResearchWorkspace[]>("/trading/research"),
  createResearchWorkspace: (body:ResearchConfig) => request<{id:string}>("/trading/research",{method:"POST",body:JSON.stringify(body)}),
  analyzeResearchWorkspace: (id:string) => request<unknown>(`/trading/research/${encodeURIComponent(id)}/analyze`,{method:"POST",body:"{}"}),
  quantStrategies: () => request<QuantCatalog>("/quant/strategies"),
  quantObservations: () => request<QuantObservation[]>("/quant/observations"),
  quantSeed: () => request<{created:number}>("/quant/seed",{method:"POST"}),
  quantRun: () => request<{inserted:number;blocked:number}>("/quant/run",{method:"POST"}),
  quantToggle: (id:string,enabled:boolean) => request<{enabled:boolean}>(`/quant/strategies/${encodeURIComponent(id)}`,{method:"PUT",body:JSON.stringify({enabled})}),
  subscription: (client_id: string, symbols: string[], dataset: string) => request<{expires_at:number;sampling:boolean}>("/data/subscriptions", {method:"POST",body:JSON.stringify({client_id,symbols,dataset})}),
  retention: () => request<Record<string,unknown>>("/data/retention"),
  cleanupPreview: () => request<{plan_id:string;expires_at:number;candidate_keys:number}>("/data/cleanup/preview", {method:"POST",body:"{}"}),
  cleanup: (plan_id:string,dry_run=true) => request<{deleted_rows?:number;dry_run:boolean}>("/data/cleanup",{method:"POST",body:JSON.stringify({plan_id,dry_run})}),
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
  indexBars: (code: string, period: string) => request<BarsResponse>(`/indices/${encodeURIComponent(code)}/bars?period=${period}`),
  breadthHistory: (period = "daily") => request<{sample: BreadthPoint[]; market: BreadthPoint[]; members: {symbol:string;name:string;industry:string}[]}>(`/market/breadth/history?period=${period}`),
  index: (code: string) => request<IndexDetail>(`/indices/${encodeURIComponent(code)}`),
  sectors: () => request<SectorRow[]>("/sectors"),
  sector: (id: number) => request<SectorDetail>(`/sectors/${id}`),
  watchWorkbench: () => request<import("./types").WatchWorkbench>("/watchlists/workbench"),
  realtimeQuotes: (symbols: string[]) => request<import("./types").RealtimeQuote[]>(`/realtime/quotes?symbols=${encodeURIComponent(symbols.join(","))}`),
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

export type BreadthPoint = {date:string; advance:number; decline:number; flat:number; phase?:string};
