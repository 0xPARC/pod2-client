/**
 * Simplified deep link system using TanStack Router directly
 * Replaces the complex parsing/validation system with direct URL conversion
 */

import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import type { Router } from "@tanstack/react-router";

/**
 * Simple deep link manager that uses TanStack Router directly
 */
export class SimpleDeepLinkManager {
  private router: Router<any, any> | null = null;
  private isListening = false;

  /**
   * Initialize with router instance
   */
  initialize(router: Router<any, any>): void {
    this.router = router;
  }

  /**
   * Start listening for deep-link events
   */
  async startListening(): Promise<void> {
    if (this.isListening || !this.router) {
      return;
    }

    try {
      await onOpenUrl((urls) => {
        console.log("[DeepLink] Received URLs:", urls);

        // Process each URL (usually there's only one)
        urls.forEach((url) => {
          this.handleIncomingDeepLink(url);
        });
      });

      this.isListening = true;
      console.log("[DeepLink] Started listening");
    } catch (error) {
      console.error("[DeepLink] Failed to start listener:", error);
      throw error;
    }
  }

  /**
   * Stop listening for deep-link events
   */
  stopListening(): void {
    this.isListening = false;
    console.log("[DeepLink] Stopped listening");
  }

  /**
   * Handle incoming deep link URL
   */
  handleIncomingDeepLink(url: string): void {
    if (!this.router) {
      console.error("[DeepLink] Router not initialized");
      return;
    }

    console.log(`[DeepLink] Processing: ${url}`);

    try {
      // Convert podnet:// URL to router path
      const routerUrl = this.convertToRouterUrl(url);
      console.log(`[DeepLink] Converted to router URL: ${routerUrl}`);

      // Let TanStack Router handle the navigation
      // Use the router's history to navigate to the URL
      window.history.pushState(null, "", routerUrl);
      this.router.invalidate();
      console.log("[DeepLink] Navigation completed");
    } catch (error) {
      console.error("[DeepLink] Navigation error:", error);
      // Fallback to documents list on error
      this.router.navigate({ to: "/documents" });
    }
  }

  /**
   * Convert podnet:// URL to TanStack Router URL
   */
  private convertToRouterUrl(deepLinkUrl: string): string {
    try {
      // Parse the URL
      const url = new URL(deepLinkUrl);

      // Validate scheme
      if (url.protocol !== "podnet:") {
        throw new Error(`Invalid scheme: ${url.protocol}`);
      }

      // Convert hostname + pathname to router path
      let routerPath = `/${url.hostname}${url.pathname}`;

      // Clean up double slashes
      routerPath = routerPath.replace(/\/+/g, "/");

      // Remove trailing slash unless it's the root
      if (routerPath.length > 1 && routerPath.endsWith("/")) {
        routerPath = routerPath.slice(0, -1);
      }

      // Add search params if present
      if (url.search) {
        routerPath += url.search;
      }

      return routerPath;
    } catch (error) {
      console.error("[DeepLink] URL conversion error:", error);
      // Return fallback path
      return "/documents";
    }
  }

  /**
   * Generate shareable deep link URL from current router state
   */
  generateShareableUrl(): string {
    if (!this.router) {
      console.warn("[DeepLink] Router not initialized for URL generation");
      return "podnet://documents";
    }

    try {
      const location = this.router.state.location;
      const path = location.pathname + (location.search || "");

      // Convert router path to podnet:// URL
      // Remove leading slash and convert to podnet:// scheme
      const deepLinkUrl = `podnet:/${path}`;

      console.log(`[DeepLink] Generated shareable URL: ${deepLinkUrl}`);
      return deepLinkUrl;
    } catch (error) {
      console.error("[DeepLink] URL generation error:", error);
      return "podnet://documents";
    }
  }

  /**
   * Generate deep link URL for specific route
   */
  generateUrlForRoute(
    to: string,
    params?: Record<string, any>,
    search?: Record<string, any>
  ): string {
    if (!this.router) {
      console.warn("[DeepLink] Router not initialized for URL generation");
      return "podnet://documents";
    }

    try {
      // Use router to build the URL with type safety
      const location = this.router.buildLocation({ to, params, search });
      const path = location.pathname + (location.search || "");

      // Convert to podnet:// URL
      const deepLinkUrl = `podnet:/${path}`;

      console.log(`[DeepLink] Generated URL for route: ${deepLinkUrl}`);
      return deepLinkUrl;
    } catch (error) {
      console.error("[DeepLink] Route URL generation error:", error);
      return "podnet://documents";
    }
  }

  /**
   * Get current listening state
   */
  get isActive(): boolean {
    return this.isListening;
  }
}

/**
 * Global instance
 */
export const simpleDeepLinkManager = new SimpleDeepLinkManager();

/**
 * Test function for deep link URLs (for debugging)
 */
export function testDeepLink(url: string): void {
  console.log(`[DeepLink] Testing URL: ${url}`);
  simpleDeepLinkManager.handleIncomingDeepLink(url);
}

/**
 * React hook for using the simplified deep link manager
 */
export function useSimpleDeepLinkManager(router: Router<any, any>) {
  return {
    initialize: () => {
      simpleDeepLinkManager.initialize(router);
    },
    start: () => simpleDeepLinkManager.startListening(),
    stop: () => simpleDeepLinkManager.stopListening(),
    generateShareableUrl: () => simpleDeepLinkManager.generateShareableUrl(),
    generateUrlForRoute: (
      to: string,
      params?: Record<string, any>,
      search?: Record<string, any>
    ) => simpleDeepLinkManager.generateUrlForRoute(to, params, search),
    isActive: simpleDeepLinkManager.isActive
  };
}
