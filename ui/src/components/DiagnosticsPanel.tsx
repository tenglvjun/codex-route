import { useMemo, useState } from "react";
import { Copy, X } from "lucide-react";
import type { DiagnosticRecord } from "../api";
import { useTranslation } from "../i18n";
import { PreferenceSelect } from "./PreferenceSelect";

type DiagnosticsPanelProps = {
  records: DiagnosticRecord[];
  onClose?: () => void;
  onOpenProviders?: () => void;
  onOpenRuntime?: () => void;
  onClear?: () => void;
};

export function DiagnosticsPanel({ records, onClose, onOpenProviders, onOpenRuntime, onClear }: DiagnosticsPanelProps) {
  const t = useTranslation();
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
        <div><p className="eyebrow">{t("diagnosticsEyebrow")}</p><h2 id="diagnostics-heading">{t("recentClientEvents")}</h2></div>
        <div className="panel-heading-actions">
          <PreferenceSelect
            value={severity}
            onChange={setSeverity}
            ariaLabel={t("filterDiagnostics")}
            options={[
              { value: "all", label: t("allSeverities") },
              { value: "error", label: t("errors") },
              { value: "warning", label: t("warnings") },
              { value: "info", label: t("info") },
            ]}
          />
          {onClear && records.length > 0 && <button className="button secondary" type="button" onClick={onClear}>{t("clear")}</button>}
          {onClose && <button className="icon-button" type="button" aria-label={t("closeDiagnostics")} onClick={onClose}><X size={18} aria-hidden="true" /></button>}
        </div>
      </div>
      {filtered.length === 0 ? (
        <div className="empty-state" role="status"><strong>{t("noDiagnostics")}</strong><p className="muted">{t("healthEventsDescription")}</p></div>
      ) : (
        <div className="diagnostics-list">
          {filtered.map((record) => (
            <article className={`diagnostic-row diagnostic-row--${record.severity}`} key={record.id}>
              <div className="diagnostic-copy">
                <div><span className="diagnostic-severity">{record.severity}</span><strong>{record.code}</strong></div>
                <p>{record.message}</p><span className="muted">{record.source} · {new Date(record.timestamp * 1000).toLocaleString()}</span>
                <div className="diagnostic-actions">
                  {record.code.startsWith("provider.") && onOpenProviders && (
                    <button className="text-button" type="button" onClick={onOpenProviders}>{t("openProviders")}</button>
                  )}
                  {record.code.startsWith("runtime.") && onOpenRuntime && (
                    <button className="text-button" type="button" onClick={onOpenRuntime}>{t("openRuntime")}</button>
                  )}
                </div>
              </div>
              <button className="icon-button" type="button" aria-label={t("copyDiagnostic", { code: record.code })} onClick={() => void copyRecord(record)}>
                <Copy size={16} aria-hidden="true" /><span className="sr-only">{copiedId === record.id ? t("copied") : t("copy")}</span>
              </button>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
