import type { AppView } from "./store";

const CURRENT_VIEW_KEY = "pod2-client-current-view";

/**
 * Load the last selected view from localStorage
 */
export function loadCurrentView(): AppView {
  try {
    const stored = localStorage.getItem(CURRENT_VIEW_KEY);
    if (stored && ["pods", "inbox", "chats", "editor"].includes(stored)) {
      return stored as AppView;
    }
  } catch (error) {
    console.warn("Failed to load current view from localStorage:", error);
  }
  return "pods"; // Default fallback
}

/**
 * Save the current view to localStorage
 */
export function saveCurrentView(view: AppView): void {
  try {
    localStorage.setItem(CURRENT_VIEW_KEY, view);
  } catch (error) {
    console.warn("Failed to save current view to localStorage:", error);
  }
}