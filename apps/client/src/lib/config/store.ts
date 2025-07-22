import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { AppConfig, FeatureConfig } from "./types";

interface ConfigStore {
  // State
  config: AppConfig | null;
  isLoading: boolean;
  error: string | null;
  isInitialized: boolean;

  // Actions
  initialize: () => Promise<void>;
  loadConfig: () => Promise<void>;
  subscribeToChanges: () => Promise<UnlistenFn>;
  reloadConfig: (configPath?: string) => Promise<void>;
  getFeature: (feature: keyof FeatureConfig) => boolean;
  getConfigSection: <K extends keyof AppConfig>(
    section: K
  ) => AppConfig[K] | null;
  clearError: () => void;
}

// Global singleton to prevent duplicate listeners - resistant to React Strict Mode
let globalInitializationPromise: Promise<void> | null = null;
let globalConfigChangedUnlisten: UnlistenFn | null = null;

// This function will only ever run once, even with React Strict Mode
async function initializeGlobalListeners(
  set: (updater: (state: ConfigStore) => Partial<ConfigStore>) => void
): Promise<void> {
  if (globalInitializationPromise) {
    // If initialization is already in progress or completed, wait for it
    return globalInitializationPromise;
  }

  console.log(
    "Frontend: Initializing global config listeners (React Strict Mode safe)"
  );

  globalInitializationPromise = (async () => {
    // Clean up any existing listeners first
    if (globalConfigChangedUnlisten) {
      globalConfigChangedUnlisten();
      globalConfigChangedUnlisten = null;
    }

    // Set up config change listener
    globalConfigChangedUnlisten = await listen("config-changed", (event) => {
      const newConfig = event.payload as AppConfig;
      console.log("Frontend: Received config-changed event:", newConfig);
      set(() => ({
        config: newConfig,
        error: null // Clear any existing errors when config updates successfully
      }));
    });
  })();

  return globalInitializationPromise;
}

export const useConfigStore = create<ConfigStore>((set, get) => ({
  // Initial state
  config: null,
  isLoading: false,
  error: null,
  isInitialized: false,

  initialize: async () => {
    const { isInitialized } = get();

    // Prevent multiple initializations
    if (isInitialized) {
      return;
    }

    try {
      set({ isLoading: true, error: null });

      // Load initial configuration
      await get().loadConfig();

      // Initialize global listeners (React Strict Mode safe)
      await initializeGlobalListeners(set);

      set({ isLoading: false, isInitialized: true });
    } catch (error) {
      set({
        error: `Failed to initialize configuration: ${error}`,
        isLoading: false
      });
    }
  },

  loadConfig: async () => {
    try {
      set({ isLoading: true, error: null });
      const config = await invoke<AppConfig>("get_app_config");
      set({ config, isLoading: false });
    } catch (error) {
      set({
        error: `Failed to load configuration: ${error}`,
        isLoading: false
      });
    }
  },

  subscribeToChanges: async () => {
    // This is handled by initializeGlobalListeners, but we provide this for compatibility
    return await listen("config-changed", (event) => {
      const newConfig = event.payload as AppConfig;
      set({ config: newConfig, error: null });
    });
  },

  reloadConfig: async (configPath?: string) => {
    try {
      set({ isLoading: true, error: null });
      const newConfig = await invoke<AppConfig>("reload_config", {
        configPath: configPath || null
      });
      set({ config: newConfig, isLoading: false });
    } catch (error) {
      set({
        error: `Failed to reload configuration: ${error}`,
        isLoading: false
      });
      throw error;
    }
  },

  getFeature: (feature: keyof FeatureConfig): boolean => {
    const { config } = get();
    return config?.features[feature] ?? false;
  },

  getConfigSection: <K extends keyof AppConfig>(
    section: K
  ): AppConfig[K] | null => {
    const { config } = get();
    return config?.[section] ?? null;
  },

  clearError: () => {
    set({ error: null });
  }
}));
