import { CircleAlert } from "lucide-react";

export function CompactStatus({ messages }: { messages: string[] }) {
  const notes = messages.filter(Boolean);
  if (!notes.length) return null;
  return <details className="compact-status"><summary aria-label={`行情状态，${notes.length} 项提示`}><CircleAlert size={14} /><span>行情待更新</span><b className="mono">{notes.length}</b></summary><div className="compact-status-content" role="status">{notes.map((note, i) => <p key={i}>{note}</p>)}</div></details>;
}
