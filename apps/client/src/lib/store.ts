import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import {
  loadEditorContent,
  saveEditorContent
} from "./features/authoring/editor";
import { executeCode, validateCode } from "./features/authoring/rpc";
import type {
  Diagnostic,
  ExecuteCodeResponse
} from "./features/authoring/types";
import { DiagnosticSeverity } from "./features/authoring/types";
import { loadCurrentView, saveCurrentView } from "./persistence";
import {
  deletePod,
  getAppState,
  getPrivateKeyInfo,
  listSpaces,
  triggerSync,
  type AppStateData,
  type PodInfo,
  type PodLists,
  type PodStats,
  type PrivateKeyInfo,
  type SpaceInfo
} from "./rpc";

// Re-export types for backward compatibility
export type { AppStateData, PodLists, PodStats, PrivateKeyInfo, SpaceInfo };

// Use the PodInfo from rpc.ts which matches the actual API
export type { PodInfo } from "./rpc";

export type AppView =
  | "pods"
  | "documents"
  | "inbox"
  | "chats"
  | "frogs"
  | "editor";
export type FolderFilter = "all" | string; // "all" or specific folder ID

interface AppStoreState {
  appState: AppStateData;
  isLoading: boolean;
  error: string | null;

  // UI State
  currentView: AppView;
  selectedFolderFilter: FolderFilter;
  selectedPodId: string | null;
  externalPodRequest: string | undefined;
  frogTimeout: number | null;

  // Folder State
  folders: SpaceInfo[];
  foldersLoading: boolean;

  // Private Key State
  privateKeyInfo: PrivateKeyInfo | null;

  // Editor State
  editorContent: string;
  editorDiagnostics: Diagnostic[];
  executionResult: ExecuteCodeResponse | null;
  executionError: string | null;
  isExecuting: boolean;
  isValidating: boolean;

  // Actions
  initialize: () => Promise<void>;
  triggerSync: () => Promise<void>;
  setError: (error: string | null) => void;
  setCurrentView: (view: AppView) => void;
  setSelectedFolderFilter: (filter: FolderFilter) => void;
  setSelectedPodId: (podId: string | null) => void;
  setExternalPodRequest: (request: string | undefined) => void;
  loadFolders: () => Promise<void>;
  setFrogTimeout: (timeout: number | null) => void;
  deletePod: (podId: string, spaceId: string) => Promise<void>;
  loadPrivateKeyInfo: () => Promise<void>;

  // Editor Actions
  setEditorContent: (content: string) => void;
  setEditorDiagnostics: (diagnostics: Diagnostic[]) => void;
  setExecutionResult: (result: ExecuteCodeResponse | null) => void;
  setExecutionError: (error: string | null) => void;
  setIsExecuting: (executing: boolean) => void;
  setIsValidating: (validating: boolean) => void;
  validateEditorCode: () => Promise<void>;
  executeEditorCode: (mock?: boolean) => Promise<void>;
  clearExecutionResults: () => void;

  // Derived getters
  getFilteredPods: () => PodInfo[];
  getPodsInFolder: (folder: String) => PodInfo[];
  getSelectedPod: () => PodInfo | null;
}

export const useAppStore = create<AppStoreState>((set, get) => ({
  appState: {
    pod_stats: {
      total_pods: 0,
      signed_pods: 0,
      main_pods: 0
    },
    pod_lists: {
      signed_pods: [],
      main_pods: []
    },
    spaces: []
  },
  isLoading: false,
  error: null,
  currentView: loadCurrentView(),
  selectedFilter: "all",
  selectedFolderFilter: "all",
  selectedPodId: null,
  externalPodRequest: undefined,
  folders: [],
  foldersLoading: false,
  frogTimeout: null,

  // Private key initial state
  privateKeyInfo: null,

  // Editor initial state
  editorContent: loadEditorContent(),
  editorDiagnostics: [],
  executionResult: null,
  executionError: null,
  isExecuting: false,
  isValidating: false,

  initialize: async () => {
    try {
      set({ isLoading: true, error: null });

      // Get initial state using type-safe RPC
      const appState = await getAppState();
      set({ appState, isLoading: false });

      console.log("appState", appState);

      // Load folders
      await get().loadFolders();

      // Load private key info
      await get().loadPrivateKeyInfo();

      // Listen for state changes from the backend
      await listen<AppStateData>("state-changed", (event) => {
        set({
          appState: event.payload,
          folders: event.payload.spaces || []
        });
        console.log("state-changed", event.payload);
      });
    } catch (error) {
      set({
        error: `Failed to initialize state: ${error}`,
        isLoading: false
      });
    }
  },

  triggerSync: async () => {
    try {
      set({ isLoading: true, error: null });
      await triggerSync();
      set({ isLoading: false });
    } catch (error) {
      set({
        error: `Failed to trigger sync: ${error}`,
        isLoading: false
      });
    }
  },

  setError: (error: string | null) => {
    set({ error });
  },

  setCurrentView: (view: AppView) => {
    set({ currentView: view, selectedPodId: null }); // Clear selected pod when changing view
    saveCurrentView(view); // Persist the view selection
  },

  setSelectedFolderFilter: (filter: FolderFilter) => {
    set({
      selectedFolderFilter: filter,
      selectedPodId: null
    }); // Clear type filter and selected pod when changing folder
  },

  setSelectedPodId: (podId: string | null) => {
    set({ selectedPodId: podId });
  },

  setExternalPodRequest: (request: string | undefined) => {
    set({ externalPodRequest: request });
  },

  setFrogTimeout: (timeout: number | null) => {
    set({ frogTimeout: timeout });
  },

  loadFolders: async () => {
    try {
      set({ foldersLoading: true });
      const folders = await listSpaces();
      set({ folders, foldersLoading: false });
    } catch (error) {
      set({
        error: `Failed to load folders: ${error}`,
        foldersLoading: false
      });
    }
  },

  loadPrivateKeyInfo: async () => {
    try {
      const privateKeyInfo = await getPrivateKeyInfo();
      set({ privateKeyInfo });
    } catch (error) {
      console.error("Failed to load private key info:", error);
      set({ privateKeyInfo: null });
    }
  },

  deletePod: async (podId: string, spaceId: string) => {
    try {
      const { appState } = get();
      const allPods = [
        ...appState.pod_lists.signed_pods,
        ...appState.pod_lists.main_pods
      ];
      const pod = allPods.find((p) => p.id === podId);

      if (pod) {
        await deletePod(spaceId, podId);
        // Trigger sync to update the UI
        await get().triggerSync();
        // Clear selected pod if it was the one being deleted
        if (get().selectedPodId === podId) {
          set({ selectedPodId: null });
        }
      }
    } catch (error) {
      set({ error: `Failed to delete pod: ${error}` });
    }
  },

  getPodsInFolder: (folder: String) => {
    const { appState } = get();

    const pods = [
      ...appState.pod_lists.signed_pods,
      ...appState.pod_lists.main_pods
    ].filter((p) => p.space === folder);

    return pods;
  },

  getFilteredPods: () => {
    const { selectedFolderFilter, appState } = get();

    if (selectedFolderFilter === "all") {
      // Return all pods when "all" is selected
      return [
        ...appState.pod_lists.signed_pods,
        ...appState.pod_lists.main_pods
      ];
    }

    return get().getPodsInFolder(selectedFolderFilter);
  },

  getSelectedPod: () => {
    const { selectedPodId } = get();
    if (!selectedPodId) return null;

    const { appState } = get();
    const allPods = [
      ...appState.pod_lists.signed_pods,
      ...appState.pod_lists.main_pods
    ];
    return allPods.find((pod) => pod.id === selectedPodId) || null;
  },

  // Editor Actions
  setEditorContent: (content: string) => {
    set({ editorContent: content });
    saveEditorContent(content);
  },

  setEditorDiagnostics: (diagnostics: Diagnostic[]) => {
    set({ editorDiagnostics: diagnostics });
  },

  setExecutionResult: (result: ExecuteCodeResponse | null) => {
    set({
      executionResult: result,
      executionError: null // Clear error when setting result
    });
  },

  setExecutionError: (error: string | null) => {
    set({
      executionError: error,
      executionResult: null // Clear result when setting error
    });
  },

  setIsExecuting: (executing: boolean) => {
    set({ isExecuting: executing });
  },

  setIsValidating: (validating: boolean) => {
    set({ isValidating: validating });
  },

  validateEditorCode: async () => {
    const { editorContent } = get();

    if (!editorContent.trim()) {
      set({ editorDiagnostics: [] });
      return;
    }

    set({ isValidating: true });
    try {
      const diagnostics = await validateCode(editorContent);
      set({ editorDiagnostics: diagnostics, isValidating: false });
    } catch (error) {
      console.error("Validation failed:", error);
      set({
        editorDiagnostics: [
          {
            message:
              error instanceof Error ? error.message : "Validation failed",
            severity: DiagnosticSeverity.Error,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1
          }
        ],
        isValidating: false
      });
    }
  },

  executeEditorCode: async (mock = false) => {
    const { editorContent, editorDiagnostics } = get();

    // Check for validation errors
    const hasErrors = editorDiagnostics.some(
      (d) => d.severity === DiagnosticSeverity.Error
    );
    if (hasErrors) {
      const firstError = editorDiagnostics.find(
        (d) => d.severity === DiagnosticSeverity.Error
      );
      const errorMessage = firstError
        ? firstError.message
        : "Code has validation errors";
      set({ executionError: `Cannot execute: ${errorMessage}` });
      return;
    }

    if (!editorContent.trim()) {
      set({ executionError: "Cannot execute empty code" });
      return;
    }

    set({
      isExecuting: true,
      executionError: null,
      executionResult: null
    });

    try {
      const result = await executeCode(editorContent, mock);
      set({
        executionResult: result,
        isExecuting: false
      });
    } catch (error) {
      console.error("Execution failed:", error);
      set({
        executionError:
          error instanceof Error ? error.message : "Execution failed",
        isExecuting: false
      });
    }
  },

  clearExecutionResults: () => {
    set({
      executionResult: null,
      executionError: null
    });
  }
}));
