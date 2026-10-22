import { useEffect, useRef, useState } from "react";
import { CandlestickSeries, ColorType, CrosshairMode, HistogramSeries, LineSeries, createChart, type IRange, type Time } from "lightweight-charts";
import type { Bar } from "../types";

function token(name: string, fallback: string) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function chartTime(date: string): Time {
  return (/^\d{10}$/.test(date) ? Number(date) : date) as Time;
}

export function KLineChart({ bars, indicator }: { bars: Bar[]; indicator: "macd" | "kdj" | "none" }) {
  const [themeRevision, setThemeRevision] = useState(0);
  useEffect(() => {
    const refresh = () => setThemeRevision((v) => v + 1);
    window.addEventListener("alphak-theme-change", refresh);
    return () => window.removeEventListener("alphak-theme-change", refresh);
  }, []);
  const mainRef = useRef<HTMLDivElement>(null);
  const subRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!mainRef.current || bars.length === 0) return;
    const paper = token("--chart-paper", "#f7f2e8");
    const ink = token("--chart-ink", "#2b2722");
    const line = token("--chart-line", "#ded5c5");
    const up = token("--chart-up", "#b23a2b");
    const down = token("--chart-down", "#2f6f5e");
    const accent = token("--chart-up", "#b23a2b");
    const upSoft = token("--chart-up-soft", "rgba(178,58,43,0.35)");
    const downSoft = token("--chart-down-soft", "rgba(47,111,94,0.35)");
    const ink3 = token("--chart-ink-4", "#8a8175");
    const gold = token("--chart-gold", "#b98a2e");
    const jade = token("--chart-jade", "#2f6f5e");

    const chart = createChart(mainRef.current, {
      width: mainRef.current.clientWidth,
      height: 420,
      layout: { background: { type: ColorType.Solid, color: paper }, textColor: ink, fontFamily: "IBM Plex Mono, monospace", fontSize: 11, attributionLogo: false },
      localization: { timeFormatter: (time: Time) => typeof time === "number" ? new Date(time * 1000).toLocaleString("zh-CN", { timeZone: "Asia/Shanghai", hour12: false }) : String(time) },
      grid: { vertLines: { color: line, style: 1 }, horzLines: { color: line, style: 1 } },
      rightPriceScale: { borderColor: line, scaleMargins: { top: 0.08, bottom: 0.22 } },
      timeScale: { borderColor: line, timeVisible: /^\d{10}$/.test(bars[0].date), secondsVisible: false, rightOffset: 4, tickMarkFormatter: (time: Time) => typeof time === "number" ? new Date(time * 1000).toLocaleTimeString("zh-CN", { timeZone: "Asia/Shanghai", hour: "2-digit", minute: "2-digit", hour12: false }) : null },
      crosshair: { mode: CrosshairMode.Normal, vertLine: { color: accent, width: 1, style: 2, labelBackgroundColor: accent }, horzLine: { color: accent, width: 1, style: 2, labelBackgroundColor: accent } },
      handleScroll: { mouseWheel: true, pressedMouseMove: true },
      handleScale: { mouseWheel: true, pinch: true },
    });

    const candle = chart.addSeries(CandlestickSeries, {
      upColor: up, downColor: down, borderUpColor: up, borderDownColor: down,
      wickUpColor: up, wickDownColor: down, priceLineVisible: false, lastValueVisible: false,
    });
    candle.setData(bars.map((b) => ({ time: chartTime(b.date), open: b.open, high: b.high, low: b.low, close: b.close })));

    const volume = chart.addSeries(HistogramSeries, { priceFormat: { type: "volume" }, priceScaleId: "volume", lastValueVisible: false, priceLineVisible: false });
    volume.priceScale().applyOptions({ scaleMargins: { top: 0.82, bottom: 0 } });
    volume.setData(bars.map((b) => ({ time: chartTime(b.date), value: b.volume, color: b.close >= b.open ? upSoft : downSoft })));

    const ma = (key: "ma5" | "ma20" | "ma60", color: string, width: 1) => {
      const series = chart.addSeries(LineSeries, { color, lineWidth: width as 1 | 2, priceLineVisible: false, lastValueVisible: false, crosshairMarkerVisible: false });
      series.setData(bars.filter((b) => b[key] !== null).map((b) => ({ time: chartTime(b.date), value: b[key] as number })));
    };
    ma("ma5", gold, 1);
    ma("ma20", jade, 1);
    ma("ma60", ink3, 1);

    chart.timeScale().fitContent();

    let subChart: ReturnType<typeof createChart> | null = null;
    let syncing = false;
    let mainRangeHandler: ((range: IRange<Time> | null) => void) | null = null;
    let subRangeHandler: ((range: IRange<Time> | null) => void) | null = null;
    const rangeText = (range: IRange<Time> | null) =>
      range ? JSON.stringify({ from: range.from, to: range.to }) : "";
    const applyRange = (target: ReturnType<typeof createChart>, range: IRange<Time> | null) => {
      if (!range || syncing) return;
      syncing = true;
      try {
        target.timeScale().setVisibleRange(range);
      } catch {
        // The target may not have data in this range (indicator warm-up); keep its current view.
      } finally {
        syncing = false;
      }
    };
    if (indicator !== "none" && subRef.current) {
      subChart = createChart(subRef.current, {
        width: subRef.current.clientWidth,
        height: 132,
        layout: { background: { type: ColorType.Solid, color: paper }, textColor: ink, fontFamily: "IBM Plex Mono, monospace", fontSize: 10, attributionLogo: false },
        grid: { vertLines: { color: line, style: 1 }, horzLines: { color: line, style: 1 } },
        rightPriceScale: { borderColor: line, scaleMargins: { top: 0.16, bottom: 0.12 } },
        timeScale: { borderColor: line, visible: false },
        crosshair: { vertLine: { color: accent, style: 2, labelBackgroundColor: accent }, horzLine: { color: accent, style: 2, labelBackgroundColor: accent } },
      });
      if (indicator === "macd") {
        const hist = subChart.addSeries(HistogramSeries, { priceLineVisible: false, lastValueVisible: false });
        hist.setData(bars.filter((b) => b.macd !== null).map((b) => ({ time: chartTime(b.date), value: b.macd as number, color: (b.macd as number) >= 0 ? upSoft : downSoft })));
        const dif = subChart.addSeries(LineSeries, { color: gold, lineWidth: 1, priceLineVisible: false, lastValueVisible: false });
        dif.setData(bars.filter((b) => b.dif !== null).map((b) => ({ time: chartTime(b.date), value: b.dif as number })));
        const dea = subChart.addSeries(LineSeries, { color: jade, lineWidth: 1, priceLineVisible: false, lastValueVisible: false });
        dea.setData(bars.filter((b) => b.dea !== null).map((b) => ({ time: chartTime(b.date), value: b.dea as number })));
      } else {
        const k = subChart.addSeries(LineSeries, { color: gold, lineWidth: 1, priceLineVisible: false, lastValueVisible: false });
        k.setData(bars.filter((b) => b.k !== null).map((b) => ({ time: chartTime(b.date), value: b.k as number })));
        const d = subChart.addSeries(LineSeries, { color: jade, lineWidth: 1, priceLineVisible: false, lastValueVisible: false });
        d.setData(bars.filter((b) => b.d !== null).map((b) => ({ time: chartTime(b.date), value: b.d as number })));
        const j = subChart.addSeries(LineSeries, { color: accent, lineWidth: 1, priceLineVisible: false, lastValueVisible: false });
        j.setData(bars.filter((b) => b.j !== null).map((b) => ({ time: chartTime(b.date), value: b.j as number })));
      }
      subChart.timeScale().fitContent();
      const initialRange = chart.timeScale().getVisibleRange();
      if (initialRange) {
        try {
          subChart.timeScale().setVisibleRange(initialRange);
        } catch {
          // The subchart may have no data in the main range (short history).
        }
        if (mainRef.current) mainRef.current.dataset.visibleRange = rangeText(initialRange);
        if (subRef.current) subRef.current.dataset.visibleRange = rangeText(initialRange);
      }
      mainRangeHandler = (range) => {
        if (mainRef.current) mainRef.current.dataset.visibleRange = rangeText(range);
        if (subChart) applyRange(subChart, range);
      };
      subRangeHandler = (range) => {
        if (subRef.current) subRef.current.dataset.visibleRange = rangeText(range);
        applyRange(chart, range);
      };
      chart.timeScale().subscribeVisibleTimeRangeChange(mainRangeHandler);
      subChart.timeScale().subscribeVisibleTimeRangeChange(subRangeHandler);
    }

    const observer = new ResizeObserver(() => {
      if (mainRef.current) chart.applyOptions({ width: mainRef.current.clientWidth });
      if (subRef.current && subChart) subChart.applyOptions({ width: subRef.current.clientWidth });
    });
    if (mainRef.current) observer.observe(mainRef.current);
    if (subRef.current) observer.observe(subRef.current);

    return () => {
      observer.disconnect();
      if (mainRangeHandler) chart.timeScale().unsubscribeVisibleTimeRangeChange(mainRangeHandler);
      if (subChart && subRangeHandler) subChart.timeScale().unsubscribeVisibleTimeRangeChange(subRangeHandler);
      chart.remove();
      subChart?.remove();
    };
  }, [bars, indicator, themeRevision]);

  return (
    <div className="chart-stack">
      <div ref={mainRef} className="chart-main" aria-label="K 线图" />
      {indicator !== "none" ? <div ref={subRef} className="chart-sub" aria-label={indicator === "macd" ? "MACD 指标" : "KDJ 指标"} /> : null}
    </div>
  );
}
