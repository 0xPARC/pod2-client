/**
 * React hooks for deep-link integration
 */

import { useEffect, useRef } from "react";
import type { DeepLinkHandler } from "./types";
import {
  deepLinkManager,
  createNavigationHandler,
  type RouterNavigationFunctions
} from "./handler";

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
 * Should be called once in the main App component with router navigation
 */
export function useDeepLinkManager(navigation?: RouterNavigationFunctions): {
  isActive: boolean;
  handlerCount: number;
  start: () => Promise<void>;
  stop: () => void;
} {
  const navigationHandlerRef = useRef<DeepLinkHandler | null>(null);
  const isInitializedRef = useRef(false);

  useEffect(() => {
    // Initialize the manager only once
    const initializeManager = async () => {
      if (!isInitializedRef.current) {
        try {
          await deepLinkManager.startListening();
          isInitializedRef.current = true;
          console.log("[DeepLink] Manager initialized successfully");
        } catch (error) {
          console.error(
            "[DeepLink] Failed to initialize deep-link manager:",
            error
          );
          return;
        }
      }

      // Remove old navigation handler if it exists
      if (navigationHandlerRef.current) {
        deepLinkManager.removeHandler(navigationHandlerRef.current);
        navigationHandlerRef.current = null;
      }

      // Register new navigation handler if navigation functions provided
      if (navigation) {
        console.log("[DeepLink] Registering navigation handler");
        const navigationHandler = createNavigationHandler(navigation);
        navigationHandlerRef.current = navigationHandler;
        deepLinkManager.addHandler(navigationHandler);
        console.log("[DeepLink] Navigation handler registered successfully");
      } else {
        console.warn("[DeepLink] No navigation functions provided");
      }
    };

    initializeManager();

    // Cleanup on unmount or navigation change
    return () => {
      if (navigationHandlerRef.current) {
        deepLinkManager.removeHandler(navigationHandlerRef.current);
        navigationHandlerRef.current = null;
      }
    };
  }, [navigation]);

  // Final cleanup on component unmount
  useEffect(() => {
    return () => {
      deepLinkManager.stopListening();
      deepLinkManager.clearHandlers();
      isInitializedRef.current = false;
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
