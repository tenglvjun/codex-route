import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FileInput, Power } from "lucide-react";
import type { ConflictPolicy, ImportCcSwitchRequest, ImportReport, ProviderSummary } from "../api";
import { displayError } from "../errors";

type ProviderPanelProps = {
  providers: ProviderSummary[];
  busy: boolean;
  onSelect: (providerId: string) => void;
  onImport: (request: ImportCcSwitchRequest) => Promise<ImportReport>;
  onError: (message: string) => void;
};

export function ProviderPanel({ providers, busy, onSelect, onImport, onError }: ProviderPanelProps) {
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("skip");
  const [importing, setImporting] = useState(false);
  const [importReport, setImportReport] = useState<ImportReport | null>(null);

  const chooseAndImport = async () => {
    let selected: string | string[] | null;
    try {
      selected = await open({
        multiple: false,
        directory: false,
        title: "Choose the cc-switch database",
        filters: [{ name: "SQLite database", extensions: ["db", "sqlite", "sqlite3"] }],
      });
    } catch (cause) {
      onError(displayError(cause));
      return;
    }
    if (typeof selected !== "string") return;

    setImporting(true);
    setImportReport(null);
    try {
      const report = await onImport({ databasePath: selected, conflictPolicy });
      setImportReport(report);
    } catch (cause) {
      onError(displayError(cause));
    } finally {
      setImporting(false);
    }
  };

  return (
    <section className="panel" aria-labelledby="providers-heading">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">PROVIDERS</p>
          <h2 id="providers-heading">Default fallback provider</h2>
        </div>
        <span className="count">{providers.length}</span>
      </div>
      <div className="provider-import">
        <label className="compact-field" htmlFor="provider-conflict-policy">
          <span>On conflict</span>
          <select
            id="provider-conflict-policy"
            value={conflictPolicy}
            onChange={(event) => setConflictPolicy(event.target.value as ConflictPolicy)}
            disabled={busy || importing}
          >
            <option value="skip">Skip existing</option>
            <option value="replace">Replace existing</option>
            <option value="rename">Import with new ID</option>
          </select>
        </label>
        <button
          className="button secondary"
          onClick={() => void chooseAndImport()}
          disabled={busy || importing}
        >
          <FileInput size={16} aria-hidden="true" />
          {importing ? "Importing..." : "Import cc-switch"}
        </button>
      </div>
      {importReport && (
        <div className="import-result" role="status" aria-live="polite">
          <strong>Import complete</strong>
          <span>
            Imported {importReport.imported} · Replaced {importReport.replaced} · Renamed {importReport.renamed} · Skipped {importReport.skipped} · Rejected {importReport.rejected.length}
          </span>
          <span className="muted import-source">{importReport.source}</span>
          {importReport.rejected.length > 0 && (
            <details>
              <summary>Rejected providers</summary>
              <ul>
                {importReport.rejected.map((provider) => (
                  <li key={`${provider.id}-${provider.reason}`}>
                    {provider.id}: {provider.reason}
                  </li>
                ))}
              </ul>
            </details>
          )}
        </div>
      )}
      {providers.length === 0 ? (
        <p className="muted">No providers available.</p>
      ) : (
        <div className="provider-list">
          {providers.map((provider) => (
            <div className="provider-row" key={provider.id}>
              <div>
                <strong>{provider.name}</strong>
                <span className="muted">{provider.id} · {provider.source}</span>
              </div>
              <button
                className={provider.isCurrent ? "badge current" : "button secondary"}
                onClick={() => onSelect(provider.id)}
                disabled={busy || provider.isCurrent}
              >
                {!provider.isCurrent && <Power size={15} aria-hidden="true" />}
                {provider.isCurrent ? "Current" : "Use provider"}
              </button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
