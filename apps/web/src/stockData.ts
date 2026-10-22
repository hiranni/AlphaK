import type { Fundamentals, Profile } from "./types";

export type SourceRow = Record<string, string | number | null>;
export function realNumber(value: unknown): number | null {
  if (value === null || value === undefined || value === "" || value === "-" || value === "--") return null;
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

export function realProfile(symbol: string, row: SourceRow, source: string, updated: number): Profile {
  const text = (key: string) => String(row[key] ?? "");
  const rawWebsite = text("ORG_WEB");
  const website = /^https?:\/\//i.test(rawWebsite) ? rawWebsite : rawWebsite && !rawWebsite.includes(":") ? `https://${rawWebsite}` : "";
  return { symbol, name: text("SECURITY_NAME_ABBR"), full_name: text("ORG_NAME"), business: text("MAIN_BUSINESS"), industry: text("BOARD_NAME_1LEVEL") || text("EM2016"), list_date: text("LISTING_DATE").slice(0, 10), controller: text("REAL_CONTROLER"), website, region: text("REGIONBK"), intro: text("ORG_PROFIE") || text("ORG_PROFILE"), source, updated_at: new Date(updated * 1000).toISOString() };
}

export function realFundamentals(symbol: string, rows: SourceRow[]): Fundamentals {
  const ratio = (numerator: number | null, denominator: number | null) => numerator != null && denominator != null && denominator !== 0 ? numerator / denominator * 100 : null;
  return {
    symbol, name: String(rows[0]?.SECURITY_NAME_ABBR ?? ""),
    reports: rows.map(row => ({ report_period: String(row.REPORT_DATE ?? "").slice(0, 10), announce_date: String(row.NOTICE_DATE ?? "").slice(0, 10), statement_type: String(row.REPORT_DATE ?? "").includes("12-31") ? "annual" : "quarterly", version: 1, effective_at: String(row.NOTICE_DATE ?? ""), supersedes: null, is_current: true, revenue: realNumber(row.TOTAL_OPERATE_INCOME ?? row.OPERATE_INCOME), net_profit: realNumber(row.PARENT_NETPROFIT), total_assets: realNumber(row.TOTAL_ASSETS), total_liabilities: realNumber(row.TOTAL_LIABILITIES), equity: realNumber(row.TOTAL_EQUITY), operating_cash_flow: realNumber(row.NETCASH_OPERATE) })),
    indicators: rows.map(row => ({ report_period: String(row.REPORT_DATE ?? "").slice(0, 10), roe: realNumber(row.ROEJQ ?? row.WEIGHTAVG_ROE), gross_margin: realNumber(row.XSMLL), net_margin: ratio(realNumber(row.PARENT_NETPROFIT), realNumber(row.TOTAL_OPERATE_INCOME ?? row.OPERATE_INCOME)), debt_ratio: realNumber(row.ZCFZL) ?? ratio(realNumber(row.TOTAL_LIABILITIES), realNumber(row.TOTAL_ASSETS)), eps: realNumber(row.EPSJB ?? row.BASIC_EPS), bps: realNumber(row.BPS) })),
    valuation: { pe_ttm: null, pb: null, pe_percentile: null, pb_percentile: null, sample_days: 0, as_of: String(rows[0]?.REPORT_DATE ?? "").slice(0, 10) },
  };
}
