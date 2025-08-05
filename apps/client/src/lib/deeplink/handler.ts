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
      return;
    }

    try {
      this.currentHandler = await onOpenUrl((urls) => {
        console.log("[DeepLink] ⚡ RECEIVED URLs:", urls);
        console.log(
          `[DeepLink] Current handler count: ${this.handlers.length}`
        );

        // Process each URL (usually there's only one)
        urls.forEach((url) => {
          this.handleDeepLinkUrl(url);
        });
      });

      this.isListening = true;
    } catch (error) {
      console.error("[DeepLink] Failed to start listener:", error);
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
    console.log(`[DeepLink] Processing: ${url}`);

    try {
      // Validate and sanitize the URL
      const validation = validateDeepLinkUrl(url);

      // Log validation issues
      if (validation.warnings.length > 0) {
        console.warn("[DeepLink] Warnings:", validation.warnings);
      }

      if (validation.errors.length > 0) {
        console.error("[DeepLink] Errors:", validation.errors);
      }

      // Use the sanitized data even if there were warnings/errors
      const deepLinkData = validation.data;

      // Additional safety check
      if (!isNavigableDeepLink(deepLinkData)) {
        console.error("[DeepLink] Not navigable, using fallback");
        const safeFallback = getSafeDeepLinkData(url);
        this.notifyHandlers(safeFallback);
        return;
      }

      // Notify all handlers
      this.notifyHandlers(deepLinkData);
    } catch (error) {
      console.error("[DeepLink] Processing error:", error);

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
      console.warn("[DeepLink] No handlers registered, ignoring URL");
      return;
    }

    this.handlers.forEach((handler, index) => {
      try {
        handler(data);
      } catch (error) {
        console.error(`[DeepLink] Handler ${index + 1} error:`, error);
      }
    });
  }
}

/**
 * Global deep-link manager instance
 */
export const deepLinkManager = new DeepLinkManager();

/**
 * Mini-app types for navigation
 */
type MiniApp = "pod-collection" | "documents" | "pod-editor" | "frogcrypto";

/**
 * Navigation functions interface for dependency injection
 */
export interface NavigationFunctions {
  setActiveApp: (app: MiniApp) => void;
  navigateToDocumentsList: () => void;
  navigateToDocument: (id: number) => void;
  navigateToDrafts: () => void;
  navigateToPublish: (
    editingDraftId?: string,
    contentType?: "document" | "link" | "file",
    replyTo?: string
  ) => void;
  navigateToDebug: () => void;
}

/**
 * Navigation handler that integrates with injected navigation functions
 */
export function createNavigationHandler(
  navigation: NavigationFunctions
): DeepLinkHandler {
  return (data: DeepLinkData) => {
    console.log(`[DeepLink] Navigating to ${data.app}`);

    try {
      // Navigate to the target app - convert string to MiniApp type
      const app = data.app as MiniApp;
      navigation.setActiveApp(app);

      // Handle app-specific navigation
      if (
        data.app === "documents" &&
        data.route &&
        data.route.app === "documents"
      ) {
        handleDocumentsNavigation(data.route.route, navigation);
      }

      console.log("[DeepLink] Navigation completed");
    } catch (error) {
      console.error("[DeepLink] Navigation error:", error);
    }
  };
}

/**
 * Handle navigation within the documents app
 */
function handleDocumentsNavigation(
  route: any,
  navigation: NavigationFunctions
): void {
  switch (route.type) {
    case "documents-list":
      navigation.navigateToDocumentsList();
      break;

    case "document-detail":
      if (route.id) {
        navigation.navigateToDocument(route.id);
      } else {
        console.warn(
          `[DeepLink] document-detail missing id, using documents-list`
        );
        navigation.navigateToDocumentsList();
      }
      break;

    case "drafts":
      navigation.navigateToDrafts();
      break;

    case "publish":
      navigation.navigateToPublish(
        route.editingDraftId,
        (route.contentType as "document" | "link" | "file") || "document",
        route.replyTo
      );
      break;

    case "debug":
      navigation.navigateToDebug();
      break;

    default:
      console.warn(`[DeepLink] Unknown route type: ${route.type}`);
      navigation.navigateToDocumentsList();
  }
}
