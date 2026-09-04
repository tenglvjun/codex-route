import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, FolderTree, Pencil, Plus, Trash2, X } from "lucide-react";
import type { ProviderSummary, UpsertRouteRuleRequest, WorkspaceRouteRule } from "../api";
import { displayError } from "../errors";

type WorkspaceRulesPanelProps = {
  providers: ProviderSummary[];
  rules: WorkspaceRouteRule[];
  busy: boolean;
  onSave: (request: UpsertRouteRuleRequest) => Promise<void>;
  onRemove: (workspace: string) => Promise<boolean>;
  onError: (message: string) => void;
  onClose?: () => void;
};

type FormErrors = {
  workspace?: string;
  provider?: string;
  form?: string;
};

function formatDate(timestamp: number) {
  if (!timestamp) return "Unknown";
  return new Date(timestamp * 1000).toLocaleString();
}

export function WorkspaceRulesPanel({
  providers,
  rules,
  busy,
  onSave,
  onRemove,
  onError,
  onClose,
}: WorkspaceRulesPanelProps) {
  const [workspace, setWorkspace] = useState("");
  const [providerId, setProviderId] = useState("");
  const [editingWorkspace, setEditingWorkspace] = useState<string | null>(null);
  const [errors, setErrors] = useState<FormErrors>({});
  const formErrorRef = useRef<HTMLParagraphElement>(null);

  const providerNames = useMemo(
    () => new Map(providers.map((provider) => [provider.id, provider.name])),
    [providers],
  );
  const selectedProviderId =
    providerId && providers.some((provider) => provider.id === providerId)
      ? providerId
      : providers.find((provider) => provider.isCurrent)?.id || providers[0]?.id || "";

  useEffect(() => {
    if (errors.form) formErrorRef.current?.focus();
  }, [errors.form]);

  const resetForm = () => {
    setWorkspace("");
    setProviderId("");
    setEditingWorkspace(null);
    setErrors({});
  };

  const editRule = (rule: WorkspaceRouteRule) => {
    setWorkspace(rule.workspace);
    setProviderId(rule.providerId);
    setEditingWorkspace(rule.workspace);
    setErrors({});
    document.getElementById("workspace-rule-provider")?.focus();
  };

  const chooseWorkspace = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Choose a Codex workspace",
      });
      if (typeof selected === "string") {
        setWorkspace(selected);
        setErrors((current) => ({ ...current, workspace: undefined, form: undefined }));
      }
    } catch (cause) {
      onError(displayError(cause));
    }
  };

  const submitRule = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const normalizedWorkspace = workspace.trim();
    if (!normalizedWorkspace) {
      setErrors({ workspace: "Workspace path is required." });
      document.getElementById("workspace-rule-workspace")?.focus();
      return;
    }
    if (!selectedProviderId) {
      setErrors({ provider: "Choose a provider for this workspace." });
      document.getElementById("workspace-rule-provider")?.focus();
      return;
    }

    setErrors({});
    try {
      await onSave({
        workspace: normalizedWorkspace,
        providerId: selectedProviderId,
        replace: editingWorkspace !== null,
      });
      resetForm();
    } catch (cause) {
      setErrors({ form: displayError(cause) });
    }
  };

  const removeRule = async (rule: WorkspaceRouteRule) => {
    let confirmed = false;
    try {
      confirmed = await confirm(`Remove the route for ${rule.workspace}?`, {
        title: "Remove workspace rule",
        kind: "warning",
        okLabel: "Remove",
        cancelLabel: "Cancel",
      });
    } catch (cause) {
      onError(displayError(cause));
      return;
    }
    if (!confirmed) return;

    let removed = false;
    try {
      removed = await onRemove(rule.workspace);
    } catch (cause) {
      onError(displayError(cause));
      return;
    }
    if (removed && editingWorkspace === rule.workspace) resetForm();
  };

  return (
    <section className="panel rules-panel" aria-labelledby="rules-heading">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">WORKSPACE ROUTING</p>
          <h2 id="rules-heading">Project provider rules</h2>
        </div>
        <div className="panel-heading-actions">
          <span className="count">{rules.length}</span>
          {onClose && (
            <button
              type="button"
              className="icon-button panel-close-button"
              onClick={onClose}
              aria-label="Close workspace rules"
              title="Close workspace rules"
            >
              <X size={18} aria-hidden="true" />
            </button>
          )}
        </div>
      </div>

      <form className="rule-form rules-form-surface" onSubmit={submitRule} noValidate>
        <div className="form-heading">
          <strong>{editingWorkspace ? "Edit workspace rule" : "Add workspace rule"}</strong>
          {editingWorkspace && (
            <button type="button" className="text-button" onClick={resetForm} disabled={busy}>
              <X size={14} aria-hidden="true" />
              Cancel edit
            </button>
          )}
        </div>
        <div className="form-grid">
          <div className="field workspace-field">
            <label htmlFor="workspace-rule-workspace">Workspace path</label>
            <div className="input-action">
              <input
                id="workspace-rule-workspace"
                type="text"
                value={workspace}
                onChange={(event) => {
                  setWorkspace(event.target.value);
                  setErrors((current) => ({ ...current, workspace: undefined, form: undefined }));
                }}
                placeholder="Choose or enter an absolute path"
                aria-describedby={errors.workspace ? "workspace-rule-workspace-error" : undefined}
                aria-invalid={errors.workspace ? "true" : undefined}
                disabled={busy}
                readOnly={editingWorkspace !== null}
              />
              <button
                type="button"
                className="icon-button"
                onClick={() => void chooseWorkspace()}
                disabled={busy || editingWorkspace !== null}
                aria-label="Choose workspace folder"
                title="Choose workspace folder"
              >
                <FolderOpen size={18} aria-hidden="true" />
              </button>
            </div>
            {errors.workspace && (
              <p className="field-error" id="workspace-rule-workspace-error">{errors.workspace}</p>
            )}
          </div>
          <div className="field">
            <label htmlFor="workspace-rule-provider">Provider</label>
            <select
              id="workspace-rule-provider"
              value={selectedProviderId}
              onChange={(event) => {
                setProviderId(event.target.value);
                setErrors((current) => ({ ...current, provider: undefined, form: undefined }));
              }}
              aria-describedby={errors.provider ? "workspace-rule-provider-error" : undefined}
              aria-invalid={errors.provider ? "true" : undefined}
              disabled={busy || providers.length === 0}
            >
              <option value="">Choose provider</option>
              {providers.map((provider) => (
                <option key={provider.id} value={provider.id}>{provider.name}</option>
              ))}
            </select>
            {errors.provider && (
              <p className="field-error" id="workspace-rule-provider-error">{errors.provider}</p>
            )}
          </div>
          <button className="button primary form-submit" type="submit" disabled={busy || providers.length === 0}>
            {editingWorkspace ? <Pencil size={15} aria-hidden="true" /> : <Plus size={16} aria-hidden="true" />}
            {editingWorkspace ? "Save rule" : "Add rule"}
          </button>
        </div>
        {errors.form && (
          <p className="form-error" ref={formErrorRef} role="alert" tabIndex={-1}>
            {errors.form}
          </p>
        )}
      </form>

      {rules.length === 0 ? (
        <div className="empty-state rules-empty-state" role="status">
          <FolderTree size={30} aria-hidden="true" />
          <div>
            <strong>No workspace rules</strong>
            <p className="muted">Add a workspace route to choose its default provider.</p>
          </div>
        </div>
      ) : (
        <div className="rules-list">
          {rules.map((rule) => (
            <div className="rule-row rule-item" key={rule.workspace}>
              <div className="rule-details rule-meta">
                <strong>{rule.workspace}</strong>
                <span className="muted">
                  {providerNames.get(rule.providerId) || rule.providerId} · Updated {formatDate(rule.updatedAt)}
                </span>
              </div>
              <div className="row-actions rule-actions">
                <button
                  className="icon-button"
                  onClick={() => editRule(rule)}
                  disabled={busy}
                  aria-label={`Edit route for ${rule.workspace}`}
                  title="Edit rule"
                >
                  <Pencil size={17} aria-hidden="true" />
                </button>
                <button
                  className="icon-button danger"
                  onClick={() => void removeRule(rule)}
                  disabled={busy}
                  aria-label={`Remove route for ${rule.workspace}`}
                  title="Remove rule"
                >
                  <Trash2 size={17} aria-hidden="true" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
