/**
 * Deep-link event handler for frontend navigation
 */

import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import type { Router } from "@tanstack/react-router";
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
 * Router-based navigation interface for deep-link integration
 */
export interface RouterNavigationFunctions {
  router: Router<any, any>;
}

/**
 * Navigation handler that uses TanStack Router for navigation
 */
export function createNavigationHandler(
  navigation: RouterNavigationFunctions
): DeepLinkHandler {
  return (data: DeepLinkData) => {
    console.log(`[DeepLink] Navigating to ${data.app}`);

    try {
      // Handle app-specific navigation using router
      if (
        data.app === "documents" &&
        data.route &&
        data.route.app === "documents"
      ) {
        handleDocumentsNavigation(data.route.route, navigation.router);
      } else {
        // Handle other apps by navigating to their base routes
        handleAppNavigation(data.app as MiniApp, navigation.router);
      }

      console.log("[DeepLink] Navigation completed");
    } catch (error) {
      console.error("[DeepLink] Navigation error:", error);
    }
  };
}

/**
 * Handle navigation to different mini-apps
 */
function handleAppNavigation(app: MiniApp, router: Router<any, any>): void {
  switch (app) {
    case "documents":
      router.navigate({ to: "/documents" });
      break;
    case "pod-collection":
      router.navigate({ to: "/pods" });
      break;
    case "pod-editor":
      router.navigate({ to: "/editor" });
      break;
    case "frogcrypto":
      router.navigate({ to: "/frogcrypto" });
      break;
    default:
      console.warn(`[DeepLink] Unknown app: ${app}`);
      router.navigate({ to: "/pods" });
  }
}

/**
 * Handle navigation within the documents app using TanStack Router
 */
function handleDocumentsNavigation(route: any, router: Router<any, any>): void {
  switch (route.type) {
    case "documents-list":
      router.navigate({ to: "/documents" });
      break;

    case "document-detail":
      if (route.id) {
        router.navigate({
          to: "/documents/document/$documentId",
          params: { documentId: route.id.toString() }
        });
      } else {
        console.warn(
          `[DeepLink] document-detail missing id, using documents-list`
        );
        router.navigate({ to: "/documents" });
      }
      break;

    case "drafts":
      router.navigate({ to: "/documents/drafts" });
      break;

    case "publish":
      router.navigate({
        to: "/documents/publish",
        search: {
          draftId: route.editingDraftId,
          contentType: route.contentType || "document",
          replyTo: route.replyTo
        }
      });
      break;

    case "debug":
      router.navigate({ to: "/debug" });
      break;

    default:
      console.warn(`[DeepLink] Unknown route type: ${route.type}`);
      router.navigate({ to: "/documents" });
  }
}
