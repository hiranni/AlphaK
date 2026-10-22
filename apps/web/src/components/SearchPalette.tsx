import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Search, CornerDownLeft } from "lucide-react";
import { api } from "../api";
import type { SearchResult } from "../types";
import { pct, changeClass } from "../format";

export function SearchPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [active, setActive] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setResults([]);
    setActive(0);
    setError("");
    const timer = window.setTimeout(() => inputRef.current?.focus(), 40);
    return () => window.clearTimeout(timer);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const trimmed = query.trim();
    if (!trimmed) {
      setResults([]);
      setLoading(false);
      return;
    }
    let current = true;
    setError("");
    setResults([]);
    setLoading(true);
    const timer = window.setTimeout(() => {
      api.search(trimmed)
        .then((items) => {
          if (!current) return;
          setResults(items);
          setActive(0);
          setError("");
        })
        .catch((err: Error) => {
          if (!current) return;
          setError(err.message);
          setResults([]);
        })
        .finally(() => { if (current) setLoading(false); });
    }, 180);
    return () => { current = false; window.clearTimeout(timer); };
  }, [query, open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.isComposing) return;
      if (event.key === "Escape") onClose();
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setActive((v) => Math.min(v + 1, Math.max(results.length - 1, 0)));
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setActive((v) => Math.max(v - 1, 0));
      }
      if (event.key === "Enter" && results[active]) {
        event.preventDefault();
        navigate(`/stock/${results[active].symbol}`);
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, results, active, navigate, onClose]);

  if (!open) return null;

  return (
    <div className="palette-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section className="palette" role="dialog" aria-modal="true" aria-label="搜索证券">
        <div className="palette-input">
          <Search size={17} aria-hidden="true" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索代码、名称或拼音首字母"
            aria-label="搜索代码、名称或拼音首字母"
            autoComplete="off"
          />
          <kbd>ESC</kbd>
        </div>
        <div className="palette-list" role="listbox" aria-label="搜索结果">
          {loading ? <div className="palette-note">正在检索…</div> : null}
          {error ? <div className="palette-note is-error">{error}</div> : null}
          {!loading && !error && query.trim() && results.length === 0 ? (
            <div className="palette-note">没有找到匹配证券。只做单标的检索，不做全市场扫描。</div>
          ) : null}
          {!query.trim() ? (
            <div className="palette-note">输入 600519、贵州茅台 或 gzmt。搜索只针对单只证券，不提供全市场筛选。</div>
          ) : null}
          {results.map((item, index) => (
            <button
              key={item.symbol}
              className={`palette-row ${index === active ? "is-active" : ""}`}
              onMouseEnter={() => setActive(index)}
              onClick={() => { navigate(`/stock/${item.symbol}`); onClose(); }}
              role="option"
              aria-selected={index === active}
            >
              <span className="palette-symbol mono">{item.symbol}</span>
              <span className="palette-name">{item.name}</span>
              <span className="palette-industry">{item.industry}</span>
              <span className={`palette-price mono ${changeClass(item.change_pct)}`}>{item.close?.toFixed(2) ?? "—"}</span>
              <span className={`palette-pct mono ${changeClass(item.change_pct)}`}>{pct(item.change_pct)}</span>
            </button>
          ))}
        </div>
        <footer className="palette-foot">
          <span><CornerDownLeft size={13} aria-hidden="true" /> 打开</span>
          <span>↑↓ 选择</span>
          <span>支持 A 股名称、代码及拼音检索</span>
        </footer>
      </section>
    </div>
  );
}
