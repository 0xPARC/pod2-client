import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import {
  getAppState,
  triggerSync,
  listSpaces,
  setPodPinned,
  type AppStateData,
  type PodStats,
  type PodLists,
  type PodInfo,
  type SpaceInfo
} from "./rpc";

// Re-export types for backward compatibility
export type { AppStateData, PodStats, PodLists, SpaceInfo };

// Use the PodInfo from rpc.ts which matches the actual API
export type { PodInfo } from "./rpc";

export type PodFilter = "all" | "signed" | "main" | "pinned";
export type AppView = "pods" | "inbox" | "chats";
export type FolderFilter = "all" | string; // "all" or specific folder ID

interface AppStoreState {
  appState: AppStateData;
  isLoading: boolean;
  error: string | null;

  // UI State
  currentView: AppView;
  selectedFilter: PodFilter;
  selectedFolderFilter: FolderFilter;
  selectedPodId: string | null;
  externalPodRequest: string | undefined;
  chatEnabled: boolean;
  
  // Folder State
  folders: SpaceInfo[];
  foldersLoading: boolean;

  // Actions
  initialize: () => Promise<void>;
  triggerSync: () => Promise<void>;
  setError: (error: string | null) => void;
  setCurrentView: (view: AppView) => void;
  setSelectedFilter: (filter: PodFilter) => void;
  setSelectedFolderFilter: (filter: FolderFilter) => void;
  setSelectedPodId: (podId: string | null) => void;
  setExternalPodRequest: (request: string | undefined) => void;
  loadFolders: () => Promise<void>;
  togglePodPinned: (podId: string, spaceId: string) => Promise<void>;

  // Derived getters
  getFilteredPods: () => PodInfo[];
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
    }
  },
  chatEnabled: true,
  isLoading: false,
  error: null,
  currentView: "pods",
  selectedFilter: "all",
  selectedFolderFilter: "all",
  selectedPodId: null,
  externalPodRequest: undefined,
  folders: [],
  foldersLoading: false,

  initialize: async () => {
    try {
      set({ isLoading: true, error: null });

      // Get initial state using type-safe RPC
      const appState = await getAppState();
      set({ appState, isLoading: false });

      console.log("appState", appState);

      // Load folders
      await get().loadFolders();

      // Listen for state changes from the backend
      await listen<AppStateData>("state-changed", (event) => {
        set({ appState: event.payload });
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
  },

  setSelectedFilter: (filter: PodFilter) => {
    set({ selectedFilter: filter, selectedPodId: null }); // Clear selected pod when changing filter
  },

  setSelectedFolderFilter: (filter: FolderFilter) => {
    set({ selectedFolderFilter: filter, selectedPodId: null }); // Clear selected pod when changing folder
  },

  setSelectedPodId: (podId: string | null) => {
    set({ selectedPodId: podId });
  },

  setExternalPodRequest: (request: string | undefined) => {
    set({ externalPodRequest: request });
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

  togglePodPinned: async (podId: string, spaceId: string) => {
    try {
      const { appState } = get();
      const allPods = [...appState.pod_lists.signed_pods, ...appState.pod_lists.main_pods];
      const pod = allPods.find(p => p.id === podId);
      
      if (pod) {
        await setPodPinned(spaceId, podId, !pod.pinned);
        // Trigger sync to update the UI
        await get().triggerSync();
      }
    } catch (error) {
      set({ error: `Failed to toggle pod pinned status: ${error}` });
    }
  },

  getFilteredPods: () => {
    const { appState, selectedFilter, selectedFolderFilter } = get();
    
    // Get all pods or filter by type
    let pods: PodInfo[] = [];
    switch (selectedFilter) {
      case "signed":
        pods = appState.pod_lists.signed_pods;
        break;
      case "main":
        pods = appState.pod_lists.main_pods;
        break;
      case "pinned":
        pods = [...appState.pod_lists.signed_pods, ...appState.pod_lists.main_pods].filter(p => p.pinned);
        break;
      case "all":
      default:
        pods = [...appState.pod_lists.signed_pods, ...appState.pod_lists.main_pods];
        break;
    }

    // Apply folder filter
    if (selectedFolderFilter !== "all") {
      pods = pods.filter(p => p.space === selectedFolderFilter);
    }

    // Sort: pinned first, then by creation date (newest first)
    return pods.sort((a, b) => {
      if (a.pinned && !b.pinned) return -1;
      if (!a.pinned && b.pinned) return 1;
      return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
    });
  },

  getSelectedPod: () => {
    const { selectedPodId } = get();
    if (!selectedPodId) return null;

    const filteredPods = get().getFilteredPods();
    return filteredPods.find((pod) => pod.id === selectedPodId) || null;
  }
}));
