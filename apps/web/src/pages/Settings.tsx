import { useEffect, useRef, useState } from "react";
import { Check, Database, HardDrive, Lock, SlidersHorizontal } from "lucide-react";
import { api } from "../api";
import type { Settings as SettingsType } from "../types";
import { ErrorState, Loading } from "../components/States";
import { applyTheme, applyColorPreference, applyDensity } from "../theme";

export function Settings() {
  const [settings, setSettings] = useState<SettingsType | null>(null);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState("");
  const saved = useRef<SettingsType | null>(null);
  const mounted = useRef(false);
  const pending = useRef<Promise<void>>(Promise.resolve());

  const applyAppearance = (value: SettingsType) => {
    applyColorPreference(value.up_color);
    applyDensity(value.density);
    applyTheme(value.theme);
  };

  useEffect(() => {
    mounted.current = true;
    let active = true;
    api.settings().then(value => { if (active) { saved.current = value; setSettings(value); } }).catch((err: Error) => { if (active) setError(err.message); });
    return () => { active = false; mounted.current = false; };
  }, []);

  useEffect(() => { if (settings) applyAppearance(settings); }, [settings?.theme, settings?.up_color, settings?.density]);

  const patch = (key: keyof SettingsType, value: string | number) => {
    if (!settings) return;
    const next = { ...settings, [key]: value };
    setSettings(next);
    applyAppearance(next);
    setSaving(true);
    setNotice("");
    const task = async () => {
      try {
        const stored = await api.updateSettings({ [key]: value });
        saved.current = stored;
        if (mounted.current) setNotice("已自动保存");
      } catch (err) {
        if (mounted.current) setNotice(`自动保存失败：${err instanceof Error ? err.message : "请稍后重试"}`);
      }
    };
    pending.current = pending.current.then(task).finally(() => { if (mounted.current) setSaving(false); });
  };

  if (error) return <ErrorState message={error} onRetry={() => location.reload()} />;
  if (!settings) return <Loading label="正在读取设置" />;

  return (
    <div className="page settings-page">
      <header className="page-head compact-head">
        <div>
          <h1 className="display">设置</h1>
        </div>
        <div className="head-aside">
          <span className="stamp">LOCAL ONLY</span>
          <div className="head-aside-row"><span className="mono muted">绑定</span><span className="mono">127.0.0.1</span></div>
          <div className="head-aside-row"><span className="mono muted">账号</span><span className="mono">单用户</span></div>
        </div>
      </header>

      <div className="settings-grid">
        <section className="panel">
          <div className="panel-head"><div><span className="eyebrow mono">APPEARANCE</span><h2>显示</h2></div><SlidersHorizontal size={17} /></div>
          <div className="form-grid">
            <label className="field">
              <span>主题</span>
              <select className="select" aria-label="主题" value={settings.theme} onChange={(event) => patch("theme", event.target.value)}>
                <option value="light">浅色终端</option>
                <option value="dark">深色终端</option>
                <option value="system">跟随系统</option>
              </select>
              <small>更改立即生效并自动保存。</small>
            </label>
            <label className="field">
              <span>涨跌颜色</span>
              <select className="select" aria-label="涨跌颜色" value={settings.up_color} onChange={(event) => patch("up_color", event.target.value)}>
                <option value="red">红涨绿跌</option>
                <option value="green">绿涨红跌</option>
              </select>
              <small>立即更新价格、涨跌幅和图表颜色。</small>
              <span className="inline-actions" aria-label="涨跌颜色预览"><span className="change is-up">上涨 +1.28%</span><span className="change is-down">下跌 -0.86%</span></span>
            </label>
            <label className="field">
              <span>默认首页</span>
              <select className="select" value={settings.default_home} onChange={(event) => patch("default_home", event.target.value)}>
                <option value="/">市场</option>
                <option value="/watchlist">自选</option>
                <option value="/data">数据</option>
              </select>
            </label>
            <label className="field">
              <span>列表密度</span>
              <select className="select" aria-label="列表密度" value={settings.density} onChange={(event) => patch("density", event.target.value)}>
                <option value="comfortable">舒适</option>
                <option value="compact">紧凑</option>
              </select>
            </label>
            <label className="field">
              <span>时区</span>
              <select className="select" value={settings.timezone} onChange={(event) => patch("timezone", event.target.value)}>
                <option value="Asia/Shanghai">Asia/Shanghai</option>
              </select>
            </label>
            <label className="field">
              <span>自选轮询</span>
              <select className="select" value={settings.poll_interval} onChange={(event) => patch("poll_interval", Number(event.target.value))}>
                <option value={0}>关闭</option>
                <option value={15}>15 秒</option>
                <option value={30}>30 秒</option>
                <option value={60}>60 秒</option>
              </select>
            </label>
          </div>
          <div className="form-actions">
            <span className="inline-notice" role="status"><Check size={14} /> {saving ? "正在自动保存…" : notice || "更改自动保存"}</span>
          </div>
        </section>

        <aside className="settings-side">
          <section className="panel">
            <div className="panel-head"><div><span className="eyebrow mono">RUNTIME</span><h2>运行边界</h2></div><Lock size={17} /></div>
            <ul className="fact-list">
              <li><span>访问范围</span><b>仅本机</b></li>
              <li><span>数据存储</span><b>SQLite</b></li>
              <li><span>行情</span><b>多源公开行情</b></li>
              <li><span>实盘</span><b>未接入</b></li>
            </ul>
            <div className="panel-note-block">不监听局域网、不提供登录、不接券商。远程访问与账号体系属于 V1。</div>
          </section>
          <section className="panel">
            <div className="panel-head"><div><span className="eyebrow mono">BACKUP</span><h2>备份与恢复</h2></div><HardDrive size={17} /></div>
            <p className="panel-footnote">MVP 使用 SQLite 单文件。备份应包含数据库与设置；恢复前先校验备份并备份当前状态。</p>
            <button className="btn btn-ghost" disabled title="后续版本提供下载">导出备份（后续）</button>
          </section>
          <section className="panel">
            <div className="panel-head"><div><span className="eyebrow mono">ABOUT</span><h2>版本</h2></div><Database size={17} /></div>
            <ul className="fact-list">
              <li><span>产品</span><b>AlphaK MVP</b></li>
              <li><span>前端</span><b>React + Vite</b></li>
              <li><span>后端</span><b>Rust + Axum</b></li>
              <li><span>数据</span><b>SQLite · 本地缓存</b></li>
            </ul>
          </section>
        </aside>
      </div>
    </div>
  );
}
