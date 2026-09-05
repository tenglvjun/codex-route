import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Check, FileInput, LoaderCircle, RefreshCw, Server, X } from "lucide-react";
import type {
  CcSwitchScanReport,
  ConflictPolicy,
  ImportCcSwitchRequest,
  ImportReport,
  ProviderSummary,
} from "../api";
import { displayError } from "../errors";
import { useTranslation } from "../i18n";
import { PreferenceSelect } from "./PreferenceSelect";

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
  const t = useTranslation();
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
    <section className="panel provider-panel" aria-label={t("providers")}>
      <header className="provider-heading">
        <div>
          <h1>{t("providers")}</h1>
          <p>{t("providersPageDescription")}</p>
        </div>
        <button
          className="button-primary provider-import-trigger"
          type="button"
          onClick={() => setImportDialogOpen(true)}
          disabled={busy || importing}
          aria-haspopup="dialog"
          aria-expanded={importDialogOpen}
          aria-controls="cc-switch-import-dialog"
          aria-label={t("importProviders")}
        >
          <FileInput size={16} aria-hidden="true" />
          {t("import")}
        </button>
      </header>

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
            <h2 id="cc-switch-import-title">{t("importFromCcSwitch")}</h2>
            </div>
            <button
              className="icon-button import-dialog-close"
              type="button"
              ref={closeButtonRef}
              aria-label={t("closeImport")}
              title={t("close")}
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
                <strong>{t("scanningCcSwitch")}</strong>
              </div>
            )}

            {!scanning && dialogError && (
              <div className="import-dialog-error" role="alert">
                <strong>{t("importFailed")}</strong>
                <span>{dialogError}</span>
                {!scanReport && (
                    <button className="button secondary" type="button" onClick={() => void scanProviders()}>
                    <RefreshCw size={15} aria-hidden="true" />
                    {t("retryScan")}
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
                    <span>{t("selectAll")}</span>
                  </label>
                  <span className="selection-count" aria-live="polite">{t("selectedCount", { count: selectedProviderIds.length })}</span>
                </div>

                {scanReport.providers.length === 0 ? (
                  <div className="import-empty-state" role="status">
                    <Server size={24} aria-hidden="true" />
                    <strong>{t("noImportableProviders")}</strong>
                  </div>
                ) : (
                  <div className="import-provider-list" aria-label={t("availableCcSwitchProviders")}>
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
                        {provider.alreadyImported && <span className="imported-badge">{t("alreadyImported")}</span>}
                      </label>
                    ))}
                  </div>
                )}

                {scanReport.rejected.length > 0 && (
                  <details className="import-rejected">
                    <summary>{t(scanReport.rejected.length === 1 ? "unavailableConfigurations" : "unavailableConfigurationsPlural", { count: scanReport.rejected.length })}</summary>
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
                <strong>{t("importComplete")}</strong>
                <span>
                  {t("importSummary", { imported: importReport.imported, replaced: importReport.replaced, renamed: importReport.renamed, skipped: importReport.skipped, rejected: importReport.rejected.length })}
                </span>
              </div>
            )}
          </div>

          <footer className="import-dialog-footer">
            {importReport ? (
              <button className="button-primary" type="button" onClick={closeImportDialog}>{t("done")}</button>
            ) : (
              <>
                <label className="compact-field import-conflict-field" htmlFor="provider-conflict-policy">
                  <span>{t("onConflict")}</span>
                  <PreferenceSelect
                    value={conflictPolicy}
                    onChange={setConflictPolicy}
                    ariaLabel={t("onConflict")}
                    disabled={scanning || importing || !scanReport}
                    options={[
                      { value: "skip", label: t("skipExisting") },
                      { value: "replace", label: t("replaceExisting") },
                      { value: "rename", label: t("importWithNewId") },
                    ]}
                  />
                </label>
                <div className="import-dialog-actions">
                  <button className="button secondary" type="button" onClick={closeImportDialog} disabled={importing}>
                    {t("cancel")}
                  </button>
                  <button
                    className="button-primary"
                    type="button"
                    onClick={() => void importSelected()}
                    disabled={scanning || importing || selectedProviderIds.length === 0}
                  >
                    {importing ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : <FileInput size={16} aria-hidden="true" />}
                    {importing ? t("importing") : t("importSelected", { count: selectedProviderIds.length })}
                  </button>
                </div>
              </>
            )}
          </footer>
        </div>
      </dialog>

      {loading ? (
        <div className="provider-list provider-skeleton-list" role="status" aria-label={t("loadingProviders")}>
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
            <strong>{t("noProviders")}</strong>
          </div>
        </div>
      ) : (
        <div className="provider-list" role="list" aria-label={t("configuredProviders")}>
          {providers.map((provider) => (
            <article
              className={`provider-item provider-row${provider.isCurrent ? " current" : ""}`}
              role="listitem"
              key={provider.id}
              data-provider-id={provider.id}
              data-provider-current={provider.isCurrent ? "true" : "false"}
            >
              <div className="provider-summary">
                <span className="provider-icon" aria-hidden="true">
                  <Server size={22} />
                  <span className={`provider-status-dot${provider.isCurrent ? " active" : ""}`} />
                </span>
                <div className="provider-copy">
                  <strong>{provider.name}</strong>
                  <span className="muted">{provider.source}</span>
                </div>
              </div>
              {provider.isCurrent ? (
                <button
                  className="provider-current-state"
                  type="button"
                  disabled
                >
                  <Check size={15} aria-hidden="true" />
                    {t("current")}
                </button>
              ) : (
                <button
                  className="button secondary provider-use-button"
                  onClick={() => onSelect(provider.id)}
                  disabled={busy}
                >
                  {t("useProvider")}
                </button>
              )}
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
