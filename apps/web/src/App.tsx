import { Route, Routes } from "react-router-dom";
import { StrategyModule, QuantOverview, QuantSection, TradingSection, tradingPages } from "./pages/QuantWorkspace";
import { Shell } from "./components/Shell";
import { Dashboard } from "./pages/Dashboard";
import { Stock } from "./pages/Stock";
import { Watchlist } from "./pages/Watchlist";
import { DataAdmin } from "./pages/DataAdmin";
import { Settings } from "./pages/Settings";
import { IndexChart } from "./pages/IndexChart";
import { BreadthDetail } from "./pages/BreadthDetail";
import { MarketDetail } from "./pages/MarketDetail";
import { NotFound } from "./pages/NotFound";

export default function App() {
  return (
    <Routes>
      <Route element={<Shell />}>
        <Route index element={<Dashboard />} />
        <Route path="stock/:symbol" element={<Stock />} />
        <Route path="index/:code" element={<IndexChart />} />
        <Route path="market/breadth" element={<BreadthDetail />} />
        <Route path="sector/:id" element={<MarketDetail kind="sector" />} />
        <Route path="watchlist" element={<Watchlist />} />
        <Route path="quant" element={<StrategyModule />}>
          <Route index element={<QuantOverview />} />
          {["strategies", "factors", "backtests", "paper", "signals"].map(section => <Route key={section} path={section} element={<QuantSection section={section} />} />)}
        </Route>
        <Route path="trading" element={<StrategyModule trading />}>
          <Route index element={<TradingSection section="" />} />
          {tradingPages.filter(p=>p.path).map(p=><Route key={p.path} path={p.path} element={<TradingSection section={p.path} />} />)}
        </Route>
        <Route path="data" element={<DataAdmin />} />
        <Route path="settings" element={<Settings />} />
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}
