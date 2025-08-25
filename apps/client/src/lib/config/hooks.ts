import { useEffect } from "react";
import { useConfigStore } from "./store";
import type { AppConfig } from "./types";

/**
 * Hook to initialize config system - should be called once at app level
 */
export function useConfigInitialization() {
  const { initialize } = useConfigStore();

  useEffect(() => {
    const init = async () => {
      await initialize();
    };

    init();

    // Global listeners are managed by the store to prevent React Strict Mode issues
    // No cleanup needed here
  }, [initialize]);
}
/**
 * Hook for accessing a specific config section
 * Only re-renders when that specific section changes
 */
export function useConfigSection<K extends keyof AppConfig>(
  section: K
): AppConfig[K] | null {
  return useConfigStore((state) => state.config?.[section] ?? null);
}

/**
 * Hook for accessing the full configuration object
 */
export function useConfig(): AppConfig | null {
  return useConfigStore((state) => state.config);
}

/**
 * Hook for config loading state and error handling
 */
export function useConfigState() {
  return useConfigStore((state) => ({
    isLoading: state.isLoading,
    error: state.error,
    isInitialized: state.isInitialized,
    clearError: state.clearError,
    reloadConfig: state.reloadConfig
  }));
}

/**
 * Hook for manual config operations (reload, etc.)
 */
export function useConfigActions() {
  return useConfigStore((state) => ({
    loadConfig: state.loadConfig,
    reloadConfig: state.reloadConfig,
    clearError: state.clearError
  }));
}
