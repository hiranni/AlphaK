import { useEffect, useRef, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { Activity, Database, Search, Settings, Star, Command, FlaskConical, Wallet } from "lucide-react";
import { SearchPalette } from "./SearchPalette";
import { api } from "../api";
import brandLogo from "../../../../logo.svg";
import { applyTheme, applyColorPreference, applyDensity } from "../theme";

const nav = [
  { to: "/", label: "市场", icon: Activity, end: true },
  { to: "/watchlist", label: "自选", icon: Star },
  { to: "/data", label: "数据", icon: Database },
  { to: "/settings", label: "设置", icon: Settings },
  { to: "/quant", label: "量化", icon: FlaskConical },
  { to: "/trading", label: "实盘", icon: Wallet },
];

export function Shell() {
  const [searchOpen, setSearchOpen] = useState(false);
  const location = useLocation();
  const navigate = useNavigate();
  const defaultHomeApplied = useRef(false);
  const initialPath = useRef(location.pathname);
  const today = new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" }).format(new Date());

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setSearchOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    api.settings()
      .then((settings) => {
        applyColorPreference(settings.up_color);
        applyDensity(settings.density);
        applyTheme(settings.theme);
        if (!defaultHomeApplied.current) {
          defaultHomeApplied.current = true;
          if (initialPath.current === "/" && settings.default_home && settings.default_home !== "/") {
            navigate(settings.default_home, { replace: true });
          }
        }
      })
      .catch(() => undefined);
  }, [navigate]);

  useEffect(() => {
    setSearchOpen(false);
    window.scrollTo({ top: 0, behavior: "instant" });
  }, [location.pathname]);

  return (
    <div className="shell">
      <a className="skip-link" href="#main">跳到主要内容</a>
      <aside className="rail" aria-label="主导航">
        <div className="brand">
          <img className="brand-mark brand-logo" src={brandLogo} alt="" />
          <div>
            <strong>AlphaK</strong>
            <span>MARKET TERMINAL</span>
          </div>
        </div>
        <nav className="rail-nav">
          {nav.map(({ to, label, icon: Icon, end }) => (
            <NavLink key={to} to={to} end={end} className={({ isActive }) => `rail-link ${isActive ? "is-active" : ""}`}>
              <Icon size={17} strokeWidth={1.7} aria-hidden="true" />
              <span>{label}</span>
            </NavLink>
          ))}
        </nav>
        <div className="rail-note">
          <span className="rail-note-label">A 股 · 日线研究</span>
        </div>
      </aside>

      <div className="workspace">
        <header className="topbar">
          <button className="search-trigger" onClick={() => setSearchOpen(true)} aria-label="打开证券搜索">
            <Search size={16} strokeWidth={1.8} aria-hidden="true" />
            <span>搜索证券</span>
            <kbd><Command size={11} aria-hidden="true" />K</kbd>
          </button>
          <div className="topbar-meta">
            <span className="demo-chip">本机研究</span>
            <span className="topbar-date mono">{today}</span>
          </div>
        </header>
        <main id="main" className="main">
          <Outlet />
        </main>
        <footer className="site-foot">
          <span>AlphaK · 本地优先的 A 股研究工作台</span>
          <span>仅供个人研究，不构成投资建议</span>
        </footer>
      </div>
      <SearchPalette open={searchOpen} onClose={() => setSearchOpen(false)} />
    </div>
  );
}
