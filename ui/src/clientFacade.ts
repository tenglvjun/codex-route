import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { desktopApi, type ClientSnapshot, type DiagnosticRecord, type RuntimeSnapshot } from "./api";

type RuntimeEvent = {
  sequence: number;
  generatedAt: number;
  event: { type: "statusChanged"; snapshot: RuntimeSnapshot };
};
type SnapshotEvent = ClientSnapshot;
type SnapshotListener = (snapshot: ClientSnapshot) => void;
type DiagnosticListener = (record: DiagnosticRecord) => void;
type DiagnosticEvent = { sequence: number; generatedAt: number; record: DiagnosticRecord };

let currentSnapshot: ClientSnapshot | null = null;
let latestSequence = 0;
let eventUnlisteners: UnlistenFn[] = [];
let eventBridgePromise: Promise<void> | null = null;
let bridgeGeneration = 0;
let subscribers = new Set<SnapshotListener>();
let diagnosticSubscribers = new Set<DiagnosticListener>();

export async function loadSnapshot(): Promise<ClientSnapshot> {
  const snapshot = await desktopApi.getClientSnapshot();
  currentSnapshot = snapshot;
  latestSequence = Math.max(latestSequence, snapshot.sequence);
  return snapshot;
}

export function getSnapshot(): ClientSnapshot | null {
  return currentSnapshot;
}

export async function subscribe(listener: SnapshotListener): Promise<() => void> {
  subscribers.add(listener);
  if (eventUnlisteners.length === 0) await startEventBridge();
  return () => {
    subscribers.delete(listener);
    if (subscribers.size === 0 && diagnosticSubscribers.size === 0) void dispose();
  };
}

export async function subscribeDiagnostics(listener: DiagnosticListener): Promise<() => void> {
  diagnosticSubscribers.add(listener);
  if (eventUnlisteners.length === 0) await startEventBridge();
  return () => {
    diagnosticSubscribers.delete(listener);
    if (subscribers.size === 0 && diagnosticSubscribers.size === 0) void dispose();
  };
}

export async function startEventBridge(): Promise<void> {
  if (eventUnlisteners.length > 0) return;
  if (eventBridgePromise) return eventBridgePromise;
  const generation = bridgeGeneration;
  const promise = installEventBridge(generation);
  eventBridgePromise = promise;
  try {
    await promise;
  } finally {
    if (eventBridgePromise === promise) eventBridgePromise = null;
  }
}

async function installEventBridge(generation: number): Promise<void> {
  const unlisteners: UnlistenFn[] = [];
  try {
    unlisteners.push(await listen<SnapshotEvent>("client-snapshot-updated", (event) => {
      const snapshot = event.payload;
      const sequence = snapshot.sequence;
      if (sequence < latestSequence) return;
      latestSequence = sequence;
      currentSnapshot = snapshot;
      subscribers.forEach((subscriber) => subscriber(snapshot));
    }));
    unlisteners.push(await listen<RuntimeEvent>("runtime-status-changed", (event) => {
      if (!currentSnapshot) return;
      const runtime = event.payload.event.snapshot;
      if (event.payload.sequence < latestSequence) return;
      latestSequence = event.payload.sequence;
      const snapshot = { ...currentSnapshot, sequence: event.payload.sequence, runtime, generatedAt: event.payload.generatedAt };
      currentSnapshot = snapshot;
      subscribers.forEach((subscriber) => subscriber(snapshot));
    }));
    unlisteners.push(await listen<DiagnosticEvent>("diagnostic-added", (event) => {
      const record = event.payload.record;
      diagnosticSubscribers.forEach((subscriber) => subscriber(record));
      if (record.severity === "info" || !currentSnapshot) return;
      const snapshot = {
        ...currentSnapshot,
        diagnostics: {
          unreadCount: currentSnapshot.diagnostics.unreadCount + 1,
          lastError: record.severity === "error" ? record.message : currentSnapshot.diagnostics.lastError,
        },
      };
      currentSnapshot = snapshot;
      subscribers.forEach((subscriber) => subscriber(snapshot));
    }));
    if (generation !== bridgeGeneration || (subscribers.size === 0 && diagnosticSubscribers.size === 0)) {
      unlisteners.forEach((unlisten) => unlisten());
      return;
    }
    eventUnlisteners = unlisteners;
  } catch (error) {
    unlisteners.forEach((unlisten) => unlisten());
    throw error;
  }
}

export async function dispose(): Promise<void> {
  bridgeGeneration += 1;
  eventUnlisteners.forEach((unlisten) => unlisten());
  eventUnlisteners = [];
  subscribers.clear();
  diagnosticSubscribers.clear();
}

export const clientFacade = {
  loadSnapshot,
  getSnapshot,
  subscribe,
  subscribeDiagnostics,
  startRuntime: desktopApi.startRuntime,
  stopRuntime: desktopApi.stopRuntime,
  setWorkspaceProvider: desktopApi.setWorkspaceProvider,
  getDiagnostics: desktopApi.getDiagnostics,
  clearDiagnostics: desktopApi.clearDiagnostics,
  dispose,
};
