import { AlertTriangle, Inbox, LoaderCircle } from "lucide-react";

export function Loading({ label = "正在读取数据" }: { label?: string }) {
  return (
    <div className="state state-loading" role="status">
      <LoaderCircle size={18} className="spin" aria-hidden="true" />
      <span>{label}</span>
    </div>
  );
}

export function Empty({ title = "暂无数据", note }: { title?: string; note?: string }) {
  return (
    <div className="state state-empty">
      <Inbox size={20} aria-hidden="true" />
      <strong>{title}</strong>
      {note ? <span>{note}</span> : null}
    </div>
  );
}

export function ErrorState({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="state state-error" role="alert">
      <AlertTriangle size={20} aria-hidden="true" />
      <div>
        <strong>读取失败</strong>
        <span>{message}</span>
      </div>
      {onRetry ? <button className="btn btn-ghost" onClick={onRetry}>重试</button> : null}
    </div>
  );
}
