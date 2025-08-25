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

export { deepLinkManager as manager, createNavigationHandler } from "./handler";

export {
  useDeepLinkManager as useManager,
  useDeepLinkHandler as useHandler,
  useDeepLinkNavigation as useNavigation
} from "./hooks";

/**
 * Test function for debugging deep-links
 */
export function testDeepLink(url: string): void {
  const { deepLinkManager } = require("./handler");
  console.log(`[DeepLink] Testing: ${url}`);
  deepLinkManager.handleDeepLinkUrl(url);
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
