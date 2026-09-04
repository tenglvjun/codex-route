// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { desktopApi, type ClientSnapshot } from "./api";
import { clientFacade, dispose, loadSnapshot, subscribe, subscribeDiagnostics } from "./clientFacade";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("./api", async () => {
  const actual = await vi.importActual<typeof import("./api")>("./api");
  return {
    ...actual,
    desktopApi: {
      ...actual.desktopApi,
      getClientSnapshot: vi.fn(),
    },
  };
});

const snapshot = (sequence: number): ClientSnapshot => ({
  schemaVersion: 1,
  sequence,
  generatedAt: 1,
  codex: {
    home: "/tmp/.codex",
    configPath: "/tmp/.codex/config.toml",
    installed: true,
    configExists: true,
    configManaged: false,
    externalModification: false,
  },
  providers: [],
  rules: [],
  runtime: {
    phase: "running",
    active: true,
    serverReachable: true,
    configManaged: true,
    externalModification: false,
    restartCount: 0,
    updatedAt: 1,
    sequence,
  },
  diagnostics: { unreadCount: 0 },
});

describe("clientFacade", () => {
  const callbacks = new Map<string, (event: { payload: unknown }) => void>();
  const unlisteners = new Map<string, ReturnType<typeof vi.fn>>();

  beforeEach(async () => {
    await dispose();
    vi.clearAllMocks();
    callbacks.clear();
    unlisteners.clear();
    vi.mocked(desktopApi.getClientSnapshot).mockResolvedValue(snapshot(1));
    vi.mocked(listen).mockImplementation(async (event, callback) => {
      const unlisten = vi.fn();
      callbacks.set(String(event), callback as (event: { payload: unknown }) => void);
      unlisteners.set(String(event), unlisten);
      return unlisten;
    });
  });

  it("loads one snapshot and subscribes to native events", async () => {
    const listener = vi.fn();
    await loadSnapshot();
    const unsubscribe = await subscribe(listener);

    expect(desktopApi.getClientSnapshot).toHaveBeenCalledOnce();
    expect(listen).toHaveBeenCalledTimes(4);
    unsubscribe();
    expect([...unlisteners.values()].every((unlisten) => unlisten.mock.calls.length === 1)).toBe(true);
    expect(await clientFacade.getSnapshot()).toEqual(snapshot(1));
  });

  it("merges runtime and workspace events and ignores stale sequences", async () => {
    await loadSnapshot();
    const listener = vi.fn();
    const unsubscribe = await subscribe(listener);

    callbacks.get("runtime-status-changed")?.({
      payload: {
        sequence: 2,
        generatedAt: 3,
        event: { type: "statusChanged", snapshot: { ...snapshot(2).runtime, sequence: 2, phase: "degraded" } },
      },
    });
    callbacks.get("runtime-status-changed")?.({
      payload: {
        sequence: 1,
        generatedAt: 2,
        event: { type: "statusChanged", snapshot: snapshot(1).runtime },
      },
    });
    callbacks.get("workspace-changed")?.({
      payload: {
        sequence: 3,
        generatedAt: 4,
        workspace: { path: "/tmp/project", exists: true, sessionId: "session-1", threadIds: [], conflictingWorkspaces: false },
      },
    });

    expect(listener).toHaveBeenCalledTimes(2);
    expect(listener.mock.calls.at(-1)?.[0]).toMatchObject({ sequence: 3, workspace: { path: "/tmp/project" } });
    unsubscribe();
  });

  it("forwards diagnostic events without rebuilding the client snapshot", async () => {
    await loadSnapshot();
    const listener = vi.fn();
    const unsubscribe = await subscribeDiagnostics(listener);
    const record = {
      id: 7,
      timestamp: 1,
      severity: "warning" as const,
      code: "runtime.recovering",
      message: "Route is recovering",
      source: "runtime",
      context: {},
    };
    callbacks.get("diagnostic-added")?.({ payload: { sequence: 7, generatedAt: 1, record } });
    expect(listener).toHaveBeenCalledWith(record);
    unsubscribe();
  });
});
