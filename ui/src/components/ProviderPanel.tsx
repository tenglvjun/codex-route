import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FileInput, LoaderCircle, Power, RefreshCw, Server, X } from "lucide-react";
import type {
  CcSwitchScanReport,
  ConflictPolicy,
  ImportCcSwitchRequest,
  ImportReport,
  ProviderSummary,
} from "../api";
import { displayError } from "../errors";

type ProviderPanelProps = {
  providers: ProviderSummary[];
  busy: boolean;
  loading?: boolean;
  onSelect: (providerId: string) => void;
  onScan: () => Promise<CcSwitchScanReport>;
  onImport: (request: ImportCcSwitchRequest) => Promise<ImportReport>;
  importOpen?: boolean;
  onImportOpenChange?: (open: boolean) => void;
};

export function ProviderPanel({
  providers,
  busy,
  loading = false,
  onSelect,
  onScan,
  onImport,
  importOpen,
  onImportOpenChange,
}: ProviderPanelProps) {
  const [internalImportOpen, setInternalImportOpen] = useState(false);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("skip");
  const [scanning, setScanning] = useState(false);
  const [importing, setImporting] = useState(false);
  const [scanReport, setScanReport] = useState<CcSwitchScanReport | null>(null);
  const [importReport, setImportReport] = useState<ImportReport | null>(null);
  const [dialogError, setDialogError] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const dialogRef = useRef<HTMLDialogElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const scannedForOpenRef = useRef(false);
  const wasOpenRef = useRef(false);
  const requestVersionRef = useRef(0);

  const importIsControlled = importOpen !== undefined;
  const importDialogOpen = importOpen ?? internalImportOpen;
  const selectedProviderIds = useMemo(
    () => scanReport?.providers.filter((provider) => selectedIds.has(provider.id)).map((provider) => provider.id) ?? [],
    [scanReport, selectedIds],
  );
  const allSelected =
    scanReport !== null &&
    scanReport.providers.length > 0 &&
    selectedProviderIds.length === scanReport.providers.length;

  const setImportDialogOpen = (open: boolean) => {
    if (importIsControlled) {
      onImportOpenChange?.(open);
      return;
    }
    setInternalImportOpen(open);
  };

  const scanProviders = useCallback(async () => {
    const version = ++requestVersionRef.current;
    setScanning(true);
    setScanReport(null);
    setImportReport(null);
    setDialogError(null);
    setSelectedIds(new Set());
    try {
      const report = await onScan();
      if (version !== requestVersionRef.current) return;
      setScanReport(report);
      setSelectedIds(new Set(report.providers.map((provider) => provider.id)));
    } catch (cause) {
      if (version === requestVersionRef.current) setDialogError(displayError(cause));
    } finally {
      if (version === requestVersionRef.current) setScanning(false);
    }
  }, [onScan]);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (importDialogOpen) {
      if (!wasOpenRef.current) {
        openerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      }
      if (dialog && !dialog.open) {
        if (typeof dialog.showModal === "function") dialog.showModal();
        else dialog.setAttribute("open", "");
      }
      closeButtonRef.current?.focus();
      if (!scannedForOpenRef.current) {
        scannedForOpenRef.current = true;
        void scanProviders();
      }
    } else {
      requestVersionRef.current += 1;
      scannedForOpenRef.current = false;
      if (dialog?.open) {
        if (typeof dialog.close === "function") dialog.close();
        else dialog.removeAttribute("open");
      }
      if (wasOpenRef.current) openerRef.current?.focus();
    }
    wasOpenRef.current = importDialogOpen;
  }, [importDialogOpen, scanProviders]);

  const toggleProvider = (providerId: string) => {
    setImportReport(null);
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(providerId)) next.delete(providerId);
      else next.add(providerId);
      return next;
    });
  };

  const toggleAll = () => {
    setImportReport(null);
    setSelectedIds(
      allSelected ? new Set() : new Set(scanReport?.providers.map((provider) => provider.id) ?? []),
    );
  };

  const importSelected = async () => {
    if (selectedProviderIds.length === 0) return;
    setImporting(true);
    setImportReport(null);
    setDialogError(null);
    try {
      const report = await onImport({ providerIds: selectedProviderIds, conflictPolicy });
      setImportReport(report);
    } catch (cause) {
      setDialogError(displayError(cause));
    } finally {
      setImporting(false);
    }
  };

  const closeImportDialog = () => {
    if (!importing) setImportDialogOpen(false);
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
      <div className="provider-toolbar provider-import">
        <button
          className="button secondary provider-import-trigger"
          type="button"
          onClick={() => setImportDialogOpen(true)}
          disabled={busy || importing}
          aria-haspopup="dialog"
          aria-expanded={importDialogOpen}
          aria-controls="cc-switch-import-dialog"
        >
          <FileInput size={16} aria-hidden="true" />
          Import cc-switch
        </button>
      </div>

      <dialog
        className="provider-import-dialog"
        id="cc-switch-import-dialog"
        ref={dialogRef}
        aria-labelledby="cc-switch-import-title"
        onCancel={(event) => {
          event.preventDefault();
          closeImportDialog();
        }}
      >
        <div className="import-dialog-shell">
          <header className="import-dialog-header">
            <div>
              <p className="eyebrow">CC-SWITCH</p>
              <h2 id="cc-switch-import-title">Import from cc-switch</h2>
            </div>
            <button
              className="icon-button import-dialog-close"
              type="button"
              ref={closeButtonRef}
              aria-label="Close import dialog"
              title="Close"
              onClick={closeImportDialog}
              disabled={importing}
            >
              <X size={18} aria-hidden="true" />
            </button>
          </header>

          <div className="import-dialog-body" aria-busy={scanning || importing}>
            {scanning && (
              <div className="import-scan-state" role="status" aria-live="polite">
                <LoaderCircle className="spin" size={24} aria-hidden="true" />
                <strong>Scanning cc-switch...</strong>
              </div>
            )}

            {!scanning && dialogError && (
              <div className="import-dialog-error" role="alert">
                <strong>Could not complete the import</strong>
                <span>{dialogError}</span>
                {!scanReport && (
                  <button className="button secondary" type="button" onClick={() => void scanProviders()}>
                    <RefreshCw size={15} aria-hidden="true" />
                    Retry scan
                  </button>
                )}
              </div>
            )}

            {!scanning && scanReport && !importReport && (
              <>
                <div className="import-selection-toolbar">
                  <label className="select-all-control">
                    <input
                      type="checkbox"
                      checked={allSelected}
                      onChange={toggleAll}
                      disabled={scanReport.providers.length === 0 || importing}
                    />
                    <span>Select all</span>
                  </label>
                  <span className="selection-count" aria-live="polite">{selectedProviderIds.length} selected</span>
                </div>

                {scanReport.providers.length === 0 ? (
                  <div className="import-empty-state" role="status">
                    <Server size={24} aria-hidden="true" />
                    <strong>No importable Codex providers found</strong>
                  </div>
                ) : (
                  <div className="import-provider-list" aria-label="Available cc-switch providers">
                    {scanReport.providers.map((provider) => (
                      <label className="import-provider-option" key={provider.id}>
                        <input
                          type="checkbox"
                          checked={selectedIds.has(provider.id)}
                          onChange={() => toggleProvider(provider.id)}
                          disabled={importing}
                          aria-label={`${provider.name} (${provider.id})`}
                        />
                        <span className="import-provider-copy">
                          <strong>{provider.name}</strong>
                          <span>{provider.id}{provider.category ? ` · ${provider.category}` : ""}</span>
                        </span>
                        {provider.alreadyImported && <span className="imported-badge">Already imported</span>}
                      </label>
                    ))}
                  </div>
                )}

                {scanReport.rejected.length > 0 && (
                  <details className="import-rejected">
                    <summary>{scanReport.rejected.length} unavailable configuration{scanReport.rejected.length === 1 ? "" : "s"}</summary>
                    <ul>
                      {scanReport.rejected.map((provider) => (
                        <li key={`${provider.id}-${provider.reason}`}>{provider.id}: {provider.reason}</li>
                      ))}
                    </ul>
                  </details>
                )}
              </>
            )}

            {importReport && (
              <div className="import-complete" role="status" aria-live="polite">
                <span className="import-complete-icon" aria-hidden="true"><FileInput size={22} /></span>
                <strong>Import complete</strong>
                <span>
                  Imported {importReport.imported} · Replaced {importReport.replaced} · Renamed {importReport.renamed} · Skipped {importReport.skipped} · Rejected {importReport.rejected.length}
                </span>
              </div>
            )}
          </div>

          <footer className="import-dialog-footer">
            {importReport ? (
              <button className="button-primary" type="button" onClick={closeImportDialog}>Done</button>
            ) : (
              <>
                <label className="compact-field import-conflict-field" htmlFor="provider-conflict-policy">
                  <span>On conflict</span>
                  <select
                    id="provider-conflict-policy"
                    value={conflictPolicy}
                    onChange={(event) => setConflictPolicy(event.target.value as ConflictPolicy)}
                    disabled={scanning || importing || !scanReport}
                  >
                    <option value="skip">Skip existing</option>
                    <option value="replace">Replace existing</option>
                    <option value="rename">Import with new ID</option>
                  </select>
                </label>
                <div className="import-dialog-actions">
                  <button className="button secondary" type="button" onClick={closeImportDialog} disabled={importing}>
                    Cancel
                  </button>
                  <button
                    className="button-primary"
                    type="button"
                    onClick={() => void importSelected()}
                    disabled={scanning || importing || selectedProviderIds.length === 0}
                  >
                    {importing ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : <FileInput size={16} aria-hidden="true" />}
                    {importing ? "Importing..." : `Import selected (${selectedProviderIds.length})`}
                  </button>
                </div>
              </>
            )}
          </footer>
        </div>
      </dialog>

      {loading ? (
        <div className="provider-list provider-skeleton-list" role="status" aria-label="Loading providers">
          {["skeleton-a", "skeleton-b", "skeleton-c"].map((id) => (
            <div className="provider-skeleton" key={id} aria-hidden="true">
              <span className="provider-skeleton-icon" />
              <span className="provider-skeleton-copy"><span /><span /></span>
              <span className="provider-skeleton-action" />
            </div>
          ))}
        </div>
      ) : providers.length === 0 ? (
        <div className="empty-state provider-empty-state" role="status">
          <span className="empty-state-icon" aria-hidden="true"><Server size={20} /></span>
          <div>
            <strong>No providers yet</strong>
            <p className="muted">No provider is available for local requests.</p>
            <button className="button-primary empty-action" type="button" onClick={() => setImportDialogOpen(true)} disabled={busy || importing}>
              <FileInput size={15} aria-hidden="true" />
              Import from cc-switch
            </button>
          </div>
        </div>
      ) : (
        <div className="provider-list">
          {providers.map((provider) => (
            <div
              className={`provider-item provider-row${provider.isCurrent ? " current" : ""}`}
              key={provider.id}
              data-provider-id={provider.id}
              data-provider-current={provider.isCurrent ? "true" : "false"}
            >
              <div className="provider-summary">
                <span className="provider-icon" aria-hidden="true"><Server size={18} /></span>
                <span className={`provider-status-dot${provider.isCurrent ? " active" : ""}`} aria-hidden="true" />
                <div>
                  <strong>{provider.name}</strong>
                  <span className="muted">{provider.id} · {provider.source}</span>
                </div>
              </div>
              <button
                className={provider.isCurrent ? "badge current current-state" : "button secondary"}
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
