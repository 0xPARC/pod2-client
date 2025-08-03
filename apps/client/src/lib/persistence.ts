import type { MiniApp } from "./store";

const ACTIVE_APP_KEY = "pod2-client-active-app";

/**
 * Load the last selected mini-app from localStorage
 */
export function loadActiveApp(): MiniApp {
  try {
    const stored = localStorage.getItem(ACTIVE_APP_KEY);
    if (
      stored &&
      ["pod-collection", "documents", "pod-editor", "frogcrypto"].includes(
        stored
      )
    ) {
      return stored as MiniApp;
    }
  } catch (error) {
    console.warn("Failed to load active app from localStorage:", error);
  }
  return "pod-collection"; // Default fallback
}

/**
 * Save the active mini-app to localStorage
 */
export function saveActiveApp(app: MiniApp): void {
  try {
    localStorage.setItem(ACTIVE_APP_KEY, app);
  } catch (error) {
    console.warn("Failed to save active app to localStorage:", error);
  }
}
