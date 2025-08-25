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
