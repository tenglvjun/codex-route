import { useMemo, useState } from "react";
import { Copy, X } from "lucide-react";
import type { DiagnosticRecord } from "../api";

type DiagnosticsPanelProps = {
  records: DiagnosticRecord[];
  onClose?: () => void;
  onOpenProviders?: () => void;
  onOpenWorkspaceRules?: () => void;
  onOpenRuntime?: () => void;
  onClear?: () => void;
};

export function DiagnosticsPanel({ records, onClose, onOpenProviders, onOpenWorkspaceRules, onOpenRuntime, onClear }: DiagnosticsPanelProps) {
  const [severity, setSeverity] = useState<"all" | DiagnosticRecord["severity"]>("all");
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const filtered = useMemo(
    () => severity === "all" ? records : records.filter((record) => record.severity === severity),
    [records, severity],
  );

  const copyRecord = async (record: DiagnosticRecord) => {
    const details = [
      `[${record.severity}] ${record.code}`,
      record.message,
      `Source: ${record.source}`,
      ...Object.entries(record.context).map(([key, value]) => `${key}: ${value}`),
    ].join("\n");
    try {
      await navigator.clipboard.writeText(details);
      setCopiedId(record.id);
      window.setTimeout(() => setCopiedId((current) => current === record.id ? null : current), 1400);
    } catch {
      setCopiedId(null);
    }
  };

  return (
    <section className="diagnostics-panel panel" aria-labelledby="diagnostics-heading">
      <div className="panel-heading">
        <div><p className="eyebrow">DIAGNOSTICS</p><h2 id="diagnostics-heading">Recent client events</h2></div>
        <div className="panel-heading-actions">
          <select aria-label="Filter diagnostics" value={severity} onChange={(event) => setSeverity(event.target.value as typeof severity)}>
            <option value="all">All severities</option><option value="error">Errors</option><option value="warning">Warnings</option><option value="info">Info</option>
          </select>
          {onClear && records.length > 0 && <button className="button secondary" type="button" onClick={onClear}>Clear</button>}
          {onClose && <button className="icon-button" type="button" aria-label="Close diagnostics" onClick={onClose}><X size={18} aria-hidden="true" /></button>}
        </div>
      </div>
      {filtered.length === 0 ? (
        <div className="empty-state" role="status"><strong>No diagnostics</strong><p className="muted">Route and provider health events will appear here.</p></div>
      ) : (
        <div className="diagnostics-list">
          {filtered.map((record) => (
            <article className={`diagnostic-row diagnostic-row--${record.severity}`} key={record.id}>
              <div className="diagnostic-copy">
                <div><span className="diagnostic-severity">{record.severity}</span><strong>{record.code}</strong></div>
                <p>{record.message}</p><span className="muted">{record.source} · {new Date(record.timestamp * 1000).toLocaleString()}</span>
                <div className="diagnostic-actions">
                  {record.code.startsWith("provider.") && onOpenProviders && (
                    <button className="text-button" type="button" onClick={onOpenProviders}>Open Providers</button>
                  )}
                  {record.code.startsWith("workspace.") && onOpenWorkspaceRules && (
                    <button className="text-button" type="button" onClick={onOpenWorkspaceRules}>Open Workspace rules</button>
                  )}
                  {record.code.startsWith("runtime.") && onOpenRuntime && (
                    <button className="text-button" type="button" onClick={onOpenRuntime}>Open Runtime</button>
                  )}
                </div>
              </div>
              <button className="icon-button" type="button" aria-label={`Copy diagnostic ${record.code}`} onClick={() => void copyRecord(record)}>
                <Copy size={16} aria-hidden="true" /><span className="sr-only">{copiedId === record.id ? "Copied" : "Copy"}</span>
              </button>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
