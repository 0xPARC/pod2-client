/**
 * Deep-link event handler for frontend navigation
 */

import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import type { DeepLinkData, DeepLinkHandler } from "./types";
import {
  validateDeepLinkUrl,
  getSafeDeepLinkData,
  isNavigableDeepLink
} from "./validator";

/**
 * Manager class for handling deep-link events and navigation
 */
export class DeepLinkManager {
  private handlers: DeepLinkHandler[] = [];
  private isListening = false;
  private currentHandler?: () => void;

  /**
   * Start listening for deep-link events
   */
  async startListening(): Promise<void> {
    if (this.isListening) {
      console.warn("Deep-link manager is already listening");
      return;
    }

    try {
      this.currentHandler = await onOpenUrl((urls) => {
        console.log("Deep-link received:", urls);

        // Process each URL (usually there's only one)
        urls.forEach((url) => {
          this.handleDeepLinkUrl(url);
        });
      });

      this.isListening = true;
      console.log("Deep-link manager started listening");
    } catch (error) {
      console.error("Failed to start deep-link listener:", error);
      throw error;
    }
  }

  /**
   * Stop listening for deep-link events
   */
  stopListening(): void {
    if (this.currentHandler) {
      // Note: Tauri's onOpenUrl doesn't provide an unlisten function directly
      // The handler will be cleaned up when the component unmounts
      this.currentHandler = undefined;
    }

    this.isListening = false;
    console.log("Deep-link manager stopped listening");
  }

  /**
   * Add a handler for deep-link events
   */
  addHandler(handler: DeepLinkHandler): void {
    this.handlers.push(handler);
  }

  /**
   * Remove a handler for deep-link events
   */
  removeHandler(handler: DeepLinkHandler): void {
    const index = this.handlers.indexOf(handler);
    if (index > -1) {
      this.handlers.splice(index, 1);
    }
  }

  /**
   * Clear all handlers
   */
  clearHandlers(): void {
    this.handlers = [];
  }

  /**
   * Manually handle a deep-link URL (useful for testing)
   */
  handleDeepLinkUrl(url: string): void {
    console.log(`Processing deep-link URL: ${url}`);

    try {
      // Validate and sanitize the URL
      const validation = validateDeepLinkUrl(url);

      // Log validation results
      if (validation.warnings.length > 0) {
        console.warn("Deep-link validation warnings:", validation.warnings);
      }

      if (validation.errors.length > 0) {
        console.error("Deep-link validation errors:", validation.errors);
      }

      // Use the sanitized data even if there were warnings/errors
      const deepLinkData = validation.data;

      // Additional safety check
      if (!isNavigableDeepLink(deepLinkData)) {
        console.error(
          "Deep-link data is not navigable, falling back to safe default"
        );
        const safeFallback = getSafeDeepLinkData(url);
        this.notifyHandlers(safeFallback);
        return;
      }

      // Notify all handlers
      this.notifyHandlers(deepLinkData);
    } catch (error) {
      console.error("Error processing deep-link URL:", error);

      // Create a safe fallback and notify handlers
      const fallbackData = getSafeDeepLinkData(url);
      this.notifyHandlers(fallbackData);
    }
  }

  /**
   * Get current listening state
   */
  get isActive(): boolean {
    return this.isListening;
  }

  /**
   * Get number of registered handlers
   */
  get handlerCount(): number {
    return this.handlers.length;
  }

  /**
   * Notify all registered handlers
   */
  private notifyHandlers(data: DeepLinkData): void {
    if (this.handlers.length === 0) {
      console.warn(
        "No deep-link handlers registered, ignoring deep-link:",
        data.originalUrl
      );
      return;
    }

    this.handlers.forEach((handler) => {
      try {
        handler(data);
      } catch (error) {
        console.error("Error in deep-link handler:", error);
      }
    });
  }
}

/**
 * Global deep-link manager instance
 */
export const deepLinkManager = new DeepLinkManager();

/**
 * Navigation handler that integrates with the app store
 */
export function createNavigationHandler(): DeepLinkHandler {
  return (data: DeepLinkData) => {
    console.log("Navigating to deep-link:", data);

    // Import the store dynamically to avoid circular dependencies
    import("../store")
      .then(({ useAppStore }) => {
        const store = useAppStore.getState();

        try {
          // Navigate to the target app
          if (store.activeApp !== data.app) {
            console.log(`Switching to app: ${data.app}`);
            store.setActiveApp(data.app);
          }

          // Handle app-specific navigation
          if (
            data.app === "documents" &&
            data.route &&
            data.route.app === "documents"
          ) {
            handleDocumentsNavigation(data.route.route, store);
          }

          console.log("Deep-link navigation completed successfully");
        } catch (error) {
          console.error("Error during deep-link navigation:", error);
        }
      })
      .catch((error) => {
        console.error(
          "Failed to import store for deep-link navigation:",
          error
        );
      });
  };
}

/**
 * Handle navigation within the documents app
 */
function handleDocumentsNavigation(route: any, store: any): void {
  // Get the documents actions from the store
  const actions = store.documentsActions;

  if (!actions) {
    console.error("Documents actions not available, cannot navigate");
    return;
  }

  switch (route.type) {
    case "documents-list":
      actions.navigateToDocumentsList();
      break;

    case "document-detail":
      if (route.id) {
        actions.navigateToDocument(route.id);
      }
      break;

    case "drafts":
      actions.navigateToDrafts();
      break;

    case "publish":
      actions.navigateToPublish(
        route.editingDraftId,
        route.contentType || "document",
        route.replyTo
      );
      break;

    case "debug":
      actions.navigateToDebug();
      break;

    default:
      console.warn(
        `Unknown documents route type: ${route.type}, falling back to documents-list`
      );
      actions.navigateToDocumentsList();
  }
}
