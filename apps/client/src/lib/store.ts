import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
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
import { loadActiveApp, saveActiveApp } from "./persistence";
import {
  deletePod,
  getAppState,
  getBuildInfo,
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

// Re-export types for convenience
export type { AppStateData, PodLists, PodStats, PrivateKeyInfo, SpaceInfo };

// Use the PodInfo from rpc.ts which matches the actual API
export type { PodInfo } from "./rpc";

// Mini-app types
export type MiniApp =
  | "pod-collection"
  | "documents"
  | "pod-editor"
  | "frogcrypto";

export type FolderFilter = "all" | string; // "all" or specific folder ID

export interface EditDocumentData {
  documentId: number;
  postId: number;
  title: string;
  content: {
    message?: string;
    file?: {
      name: string;
      content: number[];
      mime_type: string;
    };
    url?: string;
  };
  tags: string[];
  authors: string[];
  replyTo: string | null;
}

// Mini-app state types
export interface PodCollectionState {
  selectedFolderId: FolderFilter;
  selectedPodId: string | null;
}

export interface DocumentRoute {
  type: "documents-list" | "document-detail" | "drafts" | "publish" | "debug";
  id?: number;
  editingDraftId?: string;
  contentType?: "document" | "link" | "file";
  replyTo?: string;
  editDocumentData?: EditDocumentData; // Route-specific document editing data
}

export interface DocumentsState {
  browsingHistory: {
    stack: DocumentRoute[];
    currentIndex: number;
  };
  searchQuery: string;
  selectedTag: string | null;
  cachedDocuments: Record<number, any>;
}

export interface EditorTab {
  id: string;
  name: string;
  type: "new" | "existing";
  requestId?: string;
  hasUnsavedChanges: boolean;
}

export interface PodEditorState {
  openTabs: EditorTab[];
  activeTabIndex: number;
  tabContent: Record<string, string>;
  tabDiagnostics: Record<string, Diagnostic[]>;
  tabExecutionResults: Record<string, ExecuteCodeResponse | null>;
  tabExecutionErrors: Record<string, string | null>;
  tabExecutingStates: Record<string, boolean>;
  tabValidatingStates: Record<string, boolean>;
  // Single-editor state (used until tab system is implemented)
  editorContent: string;
  editorDiagnostics: Diagnostic[];
  executionResult: ExecuteCodeResponse | null;
  executionError: string | null;
  isExecuting: boolean;
  isValidating: boolean;
}

export interface FrogCryptoState {
  currentScreen: "game" | "leaderboard" | "collection";
  frogTimeout: number | null;
  gameState: any;
}

// Action types for each mini-app
export interface PodCollectionActions {
  selectFolder: (folderId: FolderFilter) => void;
  selectPod: (podId: string | null) => void;
  refreshPods: () => Promise<void>;
}

export interface DocumentsActions {
  navigateToDocument: (id: number) => void;
  navigateToDrafts: () => void;
  navigateToPublish: (
    editingDraftId?: string,
    contentType?: "document" | "link" | "file",
    replyTo?: string,
    editDocumentData?: EditDocumentData
  ) => void;
  navigateToDocumentsList: () => void;
  navigateToDebug: () => void;
  goBack: () => void;
  goForward: () => void;
  updateSearch: (query: string) => void;
  selectTag: (tag: string | null) => void;
  cacheDocument: (id: number, document: any) => void;
}

export interface PodEditorActions {
  openTab: (tab: EditorTab) => void;
  closeTab: (tabId: string) => void;
  switchToTab: (index: number) => void;
  updateTabContent: (tabId: string, content: string) => void;
  markTabUnsaved: (tabId: string) => void;
  markTabSaved: (tabId: string) => void;
  validateTabCode: (tabId: string) => Promise<void>;
  executeTabCode: (tabId: string, mock?: boolean) => Promise<void>;
  clearTabExecutionResults: (tabId: string) => void;
  // Single-editor actions (used until tab system is implemented)
  setEditorContent: (content: string) => void;
  setEditorDiagnostics: (diagnostics: Diagnostic[]) => void;
  setExecutionResult: (result: ExecuteCodeResponse | null) => void;
  setExecutionError: (error: string | null) => void;
  setIsExecuting: (executing: boolean) => void;
  setIsValidating: (validating: boolean) => void;
  validateEditorCode: () => Promise<void>;
  executeEditorCode: (mock?: boolean) => Promise<void>;
  clearExecutionResults: () => void;
}

export interface FrogCryptoActions {
  navigateToScreen: (screen: "game" | "leaderboard" | "collection") => void;
  setFrogTimeout: (timeout: number | null) => void;
}

interface AppStoreState {
  // Global app state
  activeApp: MiniApp;
  isLoading: boolean;
  error: string | null;

  // Shared data (used across mini-apps)
  appState: AppStateData;
  folders: SpaceInfo[];
  foldersLoading: boolean;
  privateKeyInfo: PrivateKeyInfo | null;
  buildInfo: string | null;

  // Mini-app specific slices
  podCollection: PodCollectionState;
  documents: DocumentsState;
  podEditor: PodEditorState;
  frogCrypto: FrogCryptoState;

  // Global actions
  initialize: () => Promise<void>;
  triggerSync: () => Promise<void>;
  setError: (error: string | null) => void;
  setActiveApp: (app: MiniApp) => void;
  loadFolders: () => Promise<void>;
  loadPrivateKeyInfo: () => Promise<void>;
  loadBuildInfo: () => Promise<void>;
  deletePod: (podId: string, spaceId: string) => Promise<void>;

  // Mini-app actions
  podCollectionActions: PodCollectionActions;
  documentsActions: DocumentsActions;
  podEditorActions: PodEditorActions;
  frogCryptoActions: FrogCryptoActions;

  // Derived getters
  getFilteredPods: () => PodInfo[];
  getPodsInFolder: (folder: String) => PodInfo[];
  getSelectedPod: () => PodInfo | null;
}

export const useAppStore = create<AppStoreState>()(
  immer((set, get) => ({
    // Global app state
    activeApp: loadActiveApp(),
    isLoading: false,
    error: null,

    // Shared data
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
    folders: [],
    foldersLoading: false,
    privateKeyInfo: null,
    buildInfo: null,

    // Mini-app state slices
    podCollection: {
      selectedFolderId: "all",
      selectedPodId: null
    },

    documents: {
      browsingHistory: {
        stack: [{ type: "documents-list" }],
        currentIndex: 0
      },
      searchQuery: "",
      selectedTag: null,
      cachedDocuments: {}
    },

    podEditor: {
      openTabs: [],
      activeTabIndex: -1,
      tabContent: {},
      tabDiagnostics: {},
      tabExecutionResults: {},
      tabExecutionErrors: {},
      tabExecutingStates: {},
      tabValidatingStates: {},
      // Legacy fields
      editorContent: loadEditorContent(),
      editorDiagnostics: [],
      executionResult: null,
      executionError: null,
      isExecuting: false,
      isValidating: false
    },

    frogCrypto: {
      currentScreen: "game",
      frogTimeout: null,
      gameState: null
    },

    initialize: async () => {
      try {
        set((state) => {
          state.isLoading = true;
          state.error = null;
        });

        // Get initial state using type-safe RPC
        const appState = await getAppState();
        set((state) => {
          state.appState = appState;
          state.isLoading = false;
        });

        console.log("appState", appState);

        // Load folders
        await get().loadFolders();

        // Load private key info
        await get().loadPrivateKeyInfo();

        // Load build info
        await get().loadBuildInfo();

        // Listen for state changes from the backend
        await listen<AppStateData>("state-changed", (event) => {
          set((state) => {
            state.appState = event.payload;
            state.folders = event.payload.spaces || [];
          });
          console.log("state-changed", event.payload);
        });
      } catch (error) {
        set((state) => {
          state.error = `Failed to initialize state: ${error}`;
          state.isLoading = false;
        });
      }
    },

    triggerSync: async () => {
      try {
        set((state) => {
          state.isLoading = true;
          state.error = null;
        });
        await triggerSync();
        set((state) => {
          state.isLoading = false;
        });
      } catch (error) {
        set((state) => {
          state.error = `Failed to trigger sync: ${error}`;
          state.isLoading = false;
        });
      }
    },

    setError: (error: string | null) => {
      set((state) => {
        state.error = error;
      });
    },

    setActiveApp: (app: MiniApp) => {
      set((state) => {
        state.activeApp = app;
      });
      saveActiveApp(app); // Persist the app selection
    },

    // Mini-app actions
    podCollectionActions: {
      selectFolder: (folderId: FolderFilter) => {
        set((state) => {
          state.podCollection.selectedFolderId = folderId;
          state.podCollection.selectedPodId = null; // Clear selection when changing folder
        });
      },

      selectPod: (podId: string | null) => {
        set((state) => {
          state.podCollection.selectedPodId = podId;
        });
      },

      refreshPods: async () => {
        await get().triggerSync();
      }
    },

    documentsActions: {
      navigateToDocument: (id: number) => {
        set((state) => {
          const newRoute: DocumentRoute = { type: "document-detail", id };
          const history = state.documents.browsingHistory;
          // Trim forward history
          history.stack.splice(history.currentIndex + 1);
          // Add new route
          history.stack.push(newRoute);
          history.currentIndex = history.stack.length - 1;
        });
      },

      navigateToDrafts: () => {
        set((state) => {
          const newRoute: DocumentRoute = { type: "drafts" };
          const history = state.documents.browsingHistory;
          // Trim forward history
          history.stack.splice(history.currentIndex + 1);
          // Add new route
          history.stack.push(newRoute);
          history.currentIndex = history.stack.length - 1;
        });
      },

      navigateToPublish: (
        editingDraftId?: string,
        contentType?: "document" | "link" | "file",
        replyTo?: string,
        editDocumentData?: EditDocumentData
      ) => {
        set((state) => {
          const newRoute: DocumentRoute = {
            type: "publish",
            editingDraftId,
            contentType: contentType || "document", // Default to document if not specified
            replyTo,
            editDocumentData
          };
          const history = state.documents.browsingHistory;
          // Trim forward history
          history.stack.splice(history.currentIndex + 1);
          // Add new route
          history.stack.push(newRoute);
          history.currentIndex = history.stack.length - 1;
        });
      },

      navigateToDocumentsList: () => {
        set((state) => {
          const newRoute: DocumentRoute = { type: "documents-list" };
          const history = state.documents.browsingHistory;
          // Trim forward history
          history.stack.splice(history.currentIndex + 1);
          // Add new route
          history.stack.push(newRoute);
          history.currentIndex = history.stack.length - 1;
        });
      },

      navigateToDebug: () => {
        set((state) => {
          const newRoute: DocumentRoute = { type: "debug" };
          const history = state.documents.browsingHistory;
          // Trim forward history
          history.stack.splice(history.currentIndex + 1);
          // Add new route
          history.stack.push(newRoute);
          history.currentIndex = history.stack.length - 1;
        });
      },

      goBack: () => {
        set((state) => {
          const history = state.documents.browsingHistory;
          if (history.currentIndex > 0) {
            history.currentIndex -= 1;
          }
        });
      },

      goForward: () => {
        set((state) => {
          const history = state.documents.browsingHistory;
          if (history.currentIndex < history.stack.length - 1) {
            history.currentIndex += 1;
          }
        });
      },

      updateSearch: (query: string) => {
        set((state) => {
          state.documents.searchQuery = query;
        });
      },

      selectTag: (tag: string | null) => {
        set((state) => {
          state.documents.selectedTag = tag;
        });
      },

      cacheDocument: (id: number, document: any) => {
        set((state) => {
          state.documents.cachedDocuments[id] = {
            document,
            cachedAt: Date.now()
          };
        });
      }
    },

    loadFolders: async () => {
      try {
        set((state) => {
          state.foldersLoading = true;
        });
        const folders = await listSpaces();
        set((state) => {
          state.folders = folders;
          state.foldersLoading = false;
        });
      } catch (error) {
        set((state) => {
          state.error = `Failed to load folders: ${error}`;
          state.foldersLoading = false;
        });
      }
    },

    loadPrivateKeyInfo: async () => {
      try {
        const privateKeyInfo = await getPrivateKeyInfo();
        set((state) => {
          state.privateKeyInfo = privateKeyInfo;
        });
      } catch (error) {
        console.error("Failed to load private key info:", error);
        set((state) => {
          state.privateKeyInfo = null;
        });
      }
    },

    loadBuildInfo: async () => {
      try {
        const buildInfo = await getBuildInfo();
        set((state) => {
          state.buildInfo = buildInfo;
        });
      } catch (error) {
        console.error("Failed to load build info:", error);
        set((state) => {
          state.buildInfo = null;
        });
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
          if (get().podCollection.selectedPodId === podId) {
            get().podCollectionActions.selectPod(null);
          }
        }
      } catch (error) {
        set((state) => {
          state.error = `Failed to delete pod: ${error}`;
        });
      }
    },

    podEditorActions: {
      openTab: (tab: EditorTab) => {
        set((state) => {
          // Check if tab already exists
          const existingIndex = state.podEditor.openTabs.findIndex(
            (t) => t.id === tab.id
          );
          if (existingIndex >= 0) {
            state.podEditor.activeTabIndex = existingIndex;
            return;
          }

          // Add new tab
          state.podEditor.openTabs.push(tab);
          state.podEditor.activeTabIndex = state.podEditor.openTabs.length - 1;

          // Initialize empty content if not exists
          if (!state.podEditor.tabContent[tab.id]) {
            state.podEditor.tabContent[tab.id] = "";
          }
        });
      },

      closeTab: (tabId: string) => {
        set((state) => {
          const tabIndex = state.podEditor.openTabs.findIndex(
            (t) => t.id === tabId
          );
          if (tabIndex === -1) return;

          // Remove tab
          state.podEditor.openTabs.splice(tabIndex, 1);

          // Clean up tab-related state
          delete state.podEditor.tabContent[tabId];
          delete state.podEditor.tabDiagnostics[tabId];
          delete state.podEditor.tabExecutionResults[tabId];
          delete state.podEditor.tabExecutionErrors[tabId];
          delete state.podEditor.tabExecutingStates[tabId];
          delete state.podEditor.tabValidatingStates[tabId];

          // Adjust active index
          if (state.podEditor.openTabs.length === 0) {
            state.podEditor.activeTabIndex = -1;
          } else if (tabIndex <= state.podEditor.activeTabIndex) {
            state.podEditor.activeTabIndex = Math.max(
              0,
              state.podEditor.activeTabIndex - 1
            );
          }
        });
      },

      switchToTab: (index: number) => {
        set((state) => {
          if (index >= 0 && index < state.podEditor.openTabs.length) {
            state.podEditor.activeTabIndex = index;
          }
        });
      },

      updateTabContent: (tabId: string, content: string) => {
        set((state) => {
          state.podEditor.tabContent[tabId] = content;
          // Mark tab as having unsaved changes
          const tab = state.podEditor.openTabs.find((t) => t.id === tabId);
          if (tab) {
            tab.hasUnsavedChanges = true;
          }
        });
      },

      markTabUnsaved: (tabId: string) => {
        set((state) => {
          const tab = state.podEditor.openTabs.find((t) => t.id === tabId);
          if (tab) {
            tab.hasUnsavedChanges = true;
          }
        });
      },

      markTabSaved: (tabId: string) => {
        set((state) => {
          const tab = state.podEditor.openTabs.find((t) => t.id === tabId);
          if (tab) {
            tab.hasUnsavedChanges = false;
          }
        });
      },

      validateTabCode: async (tabId: string) => {
        const content = get().podEditor.tabContent[tabId] || "";

        if (!content.trim()) {
          set((state) => {
            state.podEditor.tabDiagnostics[tabId] = [];
          });
          return;
        }

        set((state) => {
          state.podEditor.tabValidatingStates[tabId] = true;
        });

        try {
          const diagnostics = await validateCode(content);
          set((state) => {
            state.podEditor.tabDiagnostics[tabId] = diagnostics;
            state.podEditor.tabValidatingStates[tabId] = false;
          });
        } catch (error) {
          console.error("Validation failed:", error);
          set((state) => {
            state.podEditor.tabDiagnostics[tabId] = [
              {
                message:
                  error instanceof Error ? error.message : "Validation failed",
                severity: DiagnosticSeverity.Error,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1
              }
            ];
            state.podEditor.tabValidatingStates[tabId] = false;
          });
        }
      },

      executeTabCode: async (tabId: string, mock = false) => {
        const content = get().podEditor.tabContent[tabId] || "";
        const diagnostics = get().podEditor.tabDiagnostics[tabId] || [];

        // Check for validation errors
        const hasErrors = diagnostics.some(
          (d) => d.severity === DiagnosticSeverity.Error
        );
        if (hasErrors) {
          const firstError = diagnostics.find(
            (d) => d.severity === DiagnosticSeverity.Error
          );
          const errorMessage = firstError
            ? firstError.message
            : "Code has validation errors";
          set((state) => {
            state.podEditor.tabExecutionErrors[tabId] =
              `Cannot execute: ${errorMessage}`;
          });
          return;
        }

        if (!content.trim()) {
          set((state) => {
            state.podEditor.tabExecutionErrors[tabId] =
              "Cannot execute empty code";
          });
          return;
        }

        set((state) => {
          state.podEditor.tabExecutingStates[tabId] = true;
          state.podEditor.tabExecutionErrors[tabId] = null;
          state.podEditor.tabExecutionResults[tabId] = null;
        });

        try {
          const result = await executeCode(content, mock);
          set((state) => {
            state.podEditor.tabExecutionResults[tabId] = result;
            state.podEditor.tabExecutingStates[tabId] = false;
          });
        } catch (error) {
          console.error("Execution failed:", error);
          set((state) => {
            state.podEditor.tabExecutionErrors[tabId] =
              error instanceof Error ? error.message : "Execution failed";
            state.podEditor.tabExecutingStates[tabId] = false;
          });
        }
      },

      clearTabExecutionResults: (tabId: string) => {
        set((state) => {
          state.podEditor.tabExecutionResults[tabId] = null;
          state.podEditor.tabExecutionErrors[tabId] = null;
        });
      },

      // Legacy actions for backward compatibility
      setEditorContent: (content: string) => {
        set((state) => {
          state.podEditor.editorContent = content;
        });
        saveEditorContent(content);
      },

      setEditorDiagnostics: (diagnostics: Diagnostic[]) => {
        set((state) => {
          state.podEditor.editorDiagnostics = diagnostics;
        });
      },

      setExecutionResult: (result: ExecuteCodeResponse | null) => {
        set((state) => {
          state.podEditor.executionResult = result;
          state.podEditor.executionError = null;
        });
      },

      setExecutionError: (error: string | null) => {
        set((state) => {
          state.podEditor.executionError = error;
          state.podEditor.executionResult = null;
        });
      },

      setIsExecuting: (executing: boolean) => {
        set((state) => {
          state.podEditor.isExecuting = executing;
        });
      },

      setIsValidating: (validating: boolean) => {
        set((state) => {
          state.podEditor.isValidating = validating;
        });
      },

      validateEditorCode: async () => {
        const { editorContent } = get().podEditor;

        if (!editorContent.trim()) {
          set((state) => {
            state.podEditor.editorDiagnostics = [];
          });
          return;
        }

        set((state) => {
          state.podEditor.isValidating = true;
        });

        try {
          const diagnostics = await validateCode(editorContent);
          set((state) => {
            state.podEditor.editorDiagnostics = diagnostics;
            state.podEditor.isValidating = false;
          });
        } catch (error) {
          console.error("Validation failed:", error);
          set((state) => {
            state.podEditor.editorDiagnostics = [
              {
                message:
                  error instanceof Error ? error.message : "Validation failed",
                severity: DiagnosticSeverity.Error,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1
              }
            ];
            state.podEditor.isValidating = false;
          });
        }
      },

      executeEditorCode: async (mock = false) => {
        const { editorContent, editorDiagnostics } = get().podEditor;

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
          set((state) => {
            state.podEditor.executionError = `Cannot execute: ${errorMessage}`;
          });
          return;
        }

        if (!editorContent.trim()) {
          set((state) => {
            state.podEditor.executionError = "Cannot execute empty code";
          });
          return;
        }

        set((state) => {
          state.podEditor.isExecuting = true;
          state.podEditor.executionError = null;
          state.podEditor.executionResult = null;
        });

        try {
          const result = await executeCode(editorContent, mock);
          set((state) => {
            state.podEditor.executionResult = result;
            state.podEditor.isExecuting = false;
          });
        } catch (error) {
          console.error("Execution failed:", error);
          set((state) => {
            state.podEditor.executionError =
              error instanceof Error ? error.message : "Execution failed";
            state.podEditor.isExecuting = false;
          });
        }
      },

      clearExecutionResults: () => {
        set((state) => {
          state.podEditor.executionResult = null;
          state.podEditor.executionError = null;
        });
      }
    },

    frogCryptoActions: {
      navigateToScreen: (screen: "game" | "leaderboard" | "collection") => {
        set((state) => {
          state.frogCrypto.currentScreen = screen;
        });
      },

      setFrogTimeout: (timeout: number | null) => {
        set((state) => {
          state.frogCrypto.frogTimeout = timeout;
        });
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
      const { podCollection, appState } = get();

      if (podCollection.selectedFolderId === "all") {
        // Return all pods when "all" is selected
        return [
          ...appState.pod_lists.signed_pods,
          ...appState.pod_lists.main_pods
        ];
      }

      return get().getPodsInFolder(podCollection.selectedFolderId);
    },

    getSelectedPod: () => {
      const { podCollection } = get();
      if (!podCollection.selectedPodId) return null;

      const { appState } = get();
      const allPods = [
        ...appState.pod_lists.signed_pods,
        ...appState.pod_lists.main_pods
      ];
      return (
        allPods.find((pod) => pod.id === podCollection.selectedPodId) || null
      );
    }
  }))
);

// Mini-app specific hooks for better ergonomics
export const useDocuments = () => {
  const documents = useAppStore((state) => state.documents);
  const documentsActions = useAppStore((state) => state.documentsActions);

  // Get current route
  const currentRoute =
    documents.browsingHistory.stack[documents.browsingHistory.currentIndex];

  return {
    // State
    browsingHistory: documents.browsingHistory,
    currentRoute, // Current route with route-specific data
    searchQuery: documents.searchQuery,
    selectedTag: documents.selectedTag,
    cachedDocuments: documents.cachedDocuments,

    // Actions
    navigateToDocument: documentsActions.navigateToDocument,
    navigateToDrafts: documentsActions.navigateToDrafts,
    navigateToPublish: documentsActions.navigateToPublish,
    navigateToDocumentsList: documentsActions.navigateToDocumentsList,
    navigateToDebug: documentsActions.navigateToDebug,
    goBack: documentsActions.goBack,
    goForward: documentsActions.goForward,
    updateSearch: documentsActions.updateSearch,
    selectTag: documentsActions.selectTag,
    cacheDocument: documentsActions.cacheDocument
  };
};

export const usePodCollection = () => {
  const podCollection = useAppStore((state) => state.podCollection);
  const podCollectionActions = useAppStore(
    (state) => state.podCollectionActions
  );
  const getFilteredPods = useAppStore((state) => state.getFilteredPods);
  const getSelectedPod = useAppStore((state) => state.getSelectedPod);

  return {
    // State
    selectedFolderId: podCollection.selectedFolderId,
    selectedPodId: podCollection.selectedPodId,

    // Actions
    selectFolder: podCollectionActions.selectFolder,
    selectPod: podCollectionActions.selectPod,
    refreshPods: podCollectionActions.refreshPods,

    // Computed
    filteredPods: getFilteredPods(),
    selectedPod: getSelectedPod()
  };
};

export const useFrogCrypto = () => {
  const frogCrypto = useAppStore((state) => state.frogCrypto);
  const frogCryptoActions = useAppStore((state) => state.frogCryptoActions);

  return {
    // State
    currentScreen: frogCrypto.currentScreen,
    frogTimeout: frogCrypto.frogTimeout,
    gameState: frogCrypto.gameState,

    // Actions
    navigateToScreen: frogCryptoActions.navigateToScreen,
    setFrogTimeout: frogCryptoActions.setFrogTimeout
  };
};

export const usePodEditor = () => {
  const podEditor = useAppStore((state) => state.podEditor);
  const podEditorActions = useAppStore((state) => state.podEditorActions);

  return {
    // Tab-based state
    openTabs: podEditor.openTabs,
    activeTabIndex: podEditor.activeTabIndex,
    tabContent: podEditor.tabContent,
    tabDiagnostics: podEditor.tabDiagnostics,
    tabExecutionResults: podEditor.tabExecutionResults,
    tabExecutionErrors: podEditor.tabExecutionErrors,
    tabExecutingStates: podEditor.tabExecutingStates,
    tabValidatingStates: podEditor.tabValidatingStates,

    // Single-editor state (used until tab system is implemented)
    editorContent: podEditor.editorContent,
    editorDiagnostics: podEditor.editorDiagnostics,
    executionResult: podEditor.executionResult,
    executionError: podEditor.executionError,
    isExecuting: podEditor.isExecuting,
    isValidating: podEditor.isValidating,

    // Tab-based actions
    openTab: podEditorActions.openTab,
    closeTab: podEditorActions.closeTab,
    switchToTab: podEditorActions.switchToTab,
    updateTabContent: podEditorActions.updateTabContent,
    markTabUnsaved: podEditorActions.markTabUnsaved,
    markTabSaved: podEditorActions.markTabSaved,
    validateTabCode: podEditorActions.validateTabCode,
    executeTabCode: podEditorActions.executeTabCode,
    clearTabExecutionResults: podEditorActions.clearTabExecutionResults,

    // Single-editor actions (used until tab system is implemented)
    setEditorContent: podEditorActions.setEditorContent,
    setEditorDiagnostics: podEditorActions.setEditorDiagnostics,
    setExecutionResult: podEditorActions.setExecutionResult,
    setExecutionError: podEditorActions.setExecutionError,
    setIsExecuting: podEditorActions.setIsExecuting,
    setIsValidating: podEditorActions.setIsValidating,
    validateEditorCode: podEditorActions.validateEditorCode,
    executeEditorCode: podEditorActions.executeEditorCode,
    clearExecutionResults: podEditorActions.clearExecutionResults
  };
};
