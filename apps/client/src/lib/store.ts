import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { create } from 'zustand';

export interface PodStats {
  total_pods: number;
  signed_pods: number;
  main_pods: number;
}

export interface PodInfo {
  id: string;
  pod_type: string;
  data: any; // PodData from Rust
  label?: string;
  created_at: string;
  space: string;
}

export interface PodLists {
  signed_pods: PodInfo[];
  main_pods: PodInfo[];
}

export interface AppStateData {
  pod_stats: PodStats;
  pod_lists: PodLists;
  // Future state can be added here easily
  // user_preferences?: UserPreferences;
  // recent_operations?: Operation[];
}

export type PodFilter = 'all' | 'signed' | 'main';
export type AppView = 'pods' | 'inbox' | 'chats';

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
      main_pods: 0,
    },
    pod_lists: {
      signed_pods: [],
      main_pods: [],
    },
  },
  chatEnabled: true,
  isLoading: false,
  error: null,
  currentView: 'pods',
  selectedFilter: 'all',
  selectedPodId: null,
  externalPodRequest: undefined,

  initialize: async () => {
    try {
      set({ isLoading: true, error: null });
      
      // Get initial state
      const appState = await invoke<AppStateData>('get_app_state');
      set({ appState, isLoading: false });
      
      // Listen for state changes from the backend
      await listen<AppStateData>('state-changed', (event) => {
        set({ appState: event.payload });
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
      await invoke('trigger_sync');
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
      case 'signed':
        return appState.pod_lists.signed_pods;
      case 'main':
        return appState.pod_lists.main_pods;
      case 'all':
      default:
        return [...appState.pod_lists.signed_pods, ...appState.pod_lists.main_pods];
    }
  },

  getSelectedPod: () => {
    const { selectedPodId } = get();
    if (!selectedPodId) return null;
    
    const filteredPods = get().getFilteredPods();
    return filteredPods.find(pod => pod.id === selectedPodId) || null;
  },
}));