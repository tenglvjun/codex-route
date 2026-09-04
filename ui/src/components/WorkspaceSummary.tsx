import { FolderOpen } from "lucide-react";
import type { WorkspaceSnapshot } from "../api";

export function WorkspaceSummary({ workspace }: { workspace?: WorkspaceSnapshot }) {
  const activity = workspace?.lastActivity
    ? new Date(workspace.lastActivity * 1000).toLocaleString()
    : "No recorded activity";
  return (
    <div className="dashboard-card dashboard-card-workspace">
      <div className="dashboard-card-icon" aria-hidden="true"><FolderOpen size={18} /></div>
      <div>
        <p className="eyebrow">WORKSPACE</p>
        <strong>{workspace?.path || "Waiting for a session"}</strong>
        <span>{workspace?.sessionId ? `Session ${workspace.sessionId}` : "Session discovery is active"}</span>
        <span>{workspace ? (workspace.exists ? "Folder available" : "Folder is no longer available") : activity}</span>
        {workspace && <span>Last activity: {activity}</span>}
      </div>
    </div>
  );
}
