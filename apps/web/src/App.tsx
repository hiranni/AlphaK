import { Route, Routes } from "react-router-dom";
import { Shell } from "./components/Shell";
import { Dashboard } from "./pages/Dashboard";
import { Stock } from "./pages/Stock";
import { Watchlist } from "./pages/Watchlist";
import { DataAdmin } from "./pages/DataAdmin";
import { Settings } from "./pages/Settings";
import { MarketDetail } from "./pages/MarketDetail";
import { NotFound } from "./pages/NotFound";

export default function App() {
  return (
    <Routes>
      <Route element={<Shell />}>
        <Route index element={<Dashboard />} />
        <Route path="stock/:symbol" element={<Stock />} />
        <Route path="index/:code" element={<MarketDetail kind="index" />} />
        <Route path="sector/:id" element={<MarketDetail kind="sector" />} />
        <Route path="watchlist" element={<Watchlist />} />
        <Route path="data" element={<DataAdmin />} />
        <Route path="settings" element={<Settings />} />
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}
