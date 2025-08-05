/**
 * React hooks for deep-link integration
 */

import { useEffect } from "react";
import type { DeepLinkHandler } from "./types";
import { deepLinkManager, createNavigationHandler } from "./handler";

/**
 * React hook for registering a deep-link handler
 * The handler will be automatically registered/unregistered based on component lifecycle
 */
export function useDeepLinkHandler(handler: DeepLinkHandler): void {
  useEffect(() => {
    deepLinkManager.addHandler(handler);

    return () => {
      deepLinkManager.removeHandler(handler);
    };
  }, [handler]);
}

/**
 * React hook to initialize deep-link listening for the entire app
 * Should be called once in the main App component
 */
export function useDeepLinkManager(): {
  isActive: boolean;
  handlerCount: number;
  start: () => Promise<void>;
  stop: () => void;
} {
  useEffect(() => {
    // Auto-start listening when the hook is first used
    const startListening = async () => {
      try {
        await deepLinkManager.startListening();

        // Register the default navigation handler
        const navigationHandler = createNavigationHandler();
        deepLinkManager.addHandler(navigationHandler);

        console.log("Deep-link manager initialized successfully");
      } catch (error) {
        console.error("Failed to initialize deep-link manager:", error);
      }
    };

    startListening();

    // Cleanup on unmount
    return () => {
      deepLinkManager.stopListening();
      deepLinkManager.clearHandlers();
    };
  }, []);

  return {
    isActive: deepLinkManager.isActive,
    handlerCount: deepLinkManager.handlerCount,
    start: () => deepLinkManager.startListening(),
    stop: () => deepLinkManager.stopListening()
  };
}

/**
 * React hook to manually trigger deep-link navigation (useful for testing)
 */
export function useDeepLinkNavigation(): {
  navigate: (url: string) => void;
  isActive: boolean;
} {
  return {
    navigate: (url: string) => deepLinkManager.handleDeepLinkUrl(url),
    isActive: deepLinkManager.isActive
  };
}
