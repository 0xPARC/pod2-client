/**
 * Deep-link system entry point
 * Provides a unified interface for all deep-link functionality
 */

// Core functionality
export * from "./types";
export * from "./parser";
export * from "./validator";
export * from "./handler";
export * from "./generator";

// React integration
export * from "./hooks";

// Re-export commonly used functions with convenient names
export {
  validateDeepLinkUrl as validateUrl,
  getSafeDeepLinkData as getSafeData,
  isNavigableDeepLink as isNavigable
} from "./validator";

export {
  parseDeepLinkUrl as parseUrl,
  isValidDeepLinkUrl as isValidUrl
} from "./parser";

export {
  generateAppUrl as generateApp,
  generateDocumentsUrl as generateDocuments,
  generateCurrentStateUrl as generateCurrent,
  generateShareableUrl as generateShareable,
  generateCommonUrls as common,
  validateGeneratedUrl as validateGenerated
} from "./generator";

export {
  deepLinkManager as manager,
  createNavigationHandler as createNavigationHandler
} from "./handler";

export {
  useDeepLinkManager as useManager,
  useDeepLinkHandler as useHandler,
  useDeepLinkNavigation as useNavigation
} from "./hooks";

/**
 * Quick setup function for initializing deep-links in an app
 * This handles the most common setup scenario
 */
export async function initializeDeepLinks(): Promise<void> {
  const { deepLinkManager, createNavigationHandler } = await import(
    "./handler"
  );

  try {
    // Start listening for deep-link events
    await deepLinkManager.startListening();

    // Add the default navigation handler
    const navigationHandler = createNavigationHandler();
    deepLinkManager.addHandler(navigationHandler);

    console.log("Deep-link system initialized successfully");
  } catch (error) {
    console.error("Failed to initialize deep-link system:", error);
    throw error;
  }
}

/**
 * Cleanup function for shutting down deep-links
 */
export function cleanupDeepLinks(): void {
  const { deepLinkManager } = require("./handler");

  deepLinkManager.stopListening();
  deepLinkManager.clearHandlers();

  console.log("Deep-link system cleaned up");
}
