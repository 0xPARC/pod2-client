import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { getAppState, triggerSync, type AppStateData, type PodStats, type PodLists, type PodInfo } from "./rpc";

// Re-export types for backward compatibility
export type { AppStateData, PodStats, PodLists };

// Use the PodInfo from rpc.ts which matches the actual API
export type { PodInfo } from "./rpc";

export type PodFilter = "all" | "signed" | "main";
export type AppView = "pods" | "inbox" | "chats";

interface AppStoreState {
  appState: AppStateData;
  isLoading: boolean;
  error: string | null;

  // UI State
  currentView: AppView;
  selectedFilter: PodFilter;
  selectedPodId: string | null;
  externalPodRequest: string | undefined;
  chatEnabled: boolean;

  // Actions
  initialize: () => Promise<void>;
  triggerSync: () => Promise<void>;
  setError: (error: string | null) => void;
  setCurrentView: (view: AppView) => void;
  setSelectedFilter: (filter: PodFilter) => void;
  setSelectedPodId: (podId: string | null) => void;
  setExternalPodRequest: (request: string | undefined) => void;

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
  selectedPodId: null,
  externalPodRequest: undefined,

  initialize: async () => {
    try {
      set({ isLoading: true, error: null });

      // Get initial state using type-safe RPC
      const appState = await getAppState();
      set({ appState, isLoading: false });

      console.log("appState", appState);

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

  setSelectedPodId: (podId: string | null) => {
    set({ selectedPodId: podId });
  },

  setExternalPodRequest: (request: string | undefined) => {
    set({ externalPodRequest: request });
  },

  getFilteredPods: () => {
    const { appState, selectedFilter } = get();
    switch (selectedFilter) {
      case "signed":
        return appState.pod_lists.signed_pods;
      case "main":
        return appState.pod_lists.main_pods;
      case "all":
      default:
        return [
          ...appState.pod_lists.signed_pods,
          ...appState.pod_lists.main_pods
        ];
    }
  },

  getSelectedPod: () => {
    const { selectedPodId } = get();
    if (!selectedPodId) return null;

    const filteredPods = get().getFilteredPods();
    return filteredPods.find((pod) => pod.id === selectedPodId) || null;
  }
}));
