/**
 * URL generator utilities for creating deep-link URLs
 */

import type { DocumentRoute, MiniApp } from "../store";
import type { DeepLinkParams, GenerateUrlOptions } from "./types";

/**
 * Generate a deep-link URL for a specific mini-app
 */
export function generateAppUrl(
  app: MiniApp,
  options: GenerateUrlOptions = {}
): string {
  const { includeScheme = true, params = {} } = options;

  const baseUrl = includeScheme ? `podnet://${app}/` : `${app}/`;

  if (Object.keys(params).length === 0) {
    return baseUrl;
  }

  const searchParams = new URLSearchParams();
  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined) {
      searchParams.set(key, value);
    }
  });

  return `${baseUrl}?${searchParams.toString()}`;
}

/**
 * Generate a deep-link URL for the documents app with a specific route
 */
export function generateDocumentsUrl(
  route: DocumentRoute,
  options: GenerateUrlOptions = {}
): string {
  const { includeScheme = true, params = {} } = options;

  let path: string;
  let routeParams: DeepLinkParams = { ...params };

  switch (route.type) {
    case "documents-list":
      path = "/";
      break;

    case "document-detail":
      if (!route.id) {
        throw new Error("Document detail route requires an ID");
      }
      path = `/document/${route.id}`;
      break;

    case "drafts":
      path = "/drafts";
      break;

    case "publish":
      path = "/publish";

      // Add route-specific parameters
      if (route.editingDraftId) {
        routeParams.editingDraftId = route.editingDraftId;
      }
      if (route.contentType) {
        routeParams.contentType = route.contentType;
      }
      if (route.replyTo) {
        routeParams.replyTo = route.replyTo;
      }
      break;

    case "debug":
      path = "/debug";
      break;

    default:
      throw new Error(`Unknown documents route type: ${(route as any).type}`);
  }

  const baseUrl = includeScheme
    ? `podnet://documents${path}`
    : `documents${path}`;

  if (Object.keys(routeParams).length === 0) {
    return baseUrl;
  }

  const searchParams = new URLSearchParams();
  Object.entries(routeParams).forEach(([key, value]) => {
    if (value !== undefined) {
      searchParams.set(key, value);
    }
  });

  return `${baseUrl}?${searchParams.toString()}`;
}

/**
 * Generate a deep-link URL from current application state
 */
export function generateCurrentStateUrl(
  options: GenerateUrlOptions = {}
): string {
  // Import the store dynamically to avoid circular dependencies
  try {
    const { useAppStore } = require("../store");
    const state = useAppStore.getState();

    const app = state.activeApp;

    if (app === "documents") {
      const documentsState = state.documentsState;
      const currentRoute =
        documentsState.browsingHistory.stack[
          documentsState.browsingHistory.currentIndex
        ];

      if (currentRoute) {
        return generateDocumentsUrl(currentRoute, options);
      }
    }

    // Fallback to simple app URL for other apps
    return generateAppUrl(app, options);
  } catch (error) {
    console.error("Failed to generate URL from current state:", error);
    return generateAppUrl("documents", options);
  }
}

/**
 * Generate a shareable deep-link URL (always includes scheme)
 */
export function generateShareableUrl(
  app: MiniApp,
  route?: DocumentRoute,
  params?: DeepLinkParams
): string {
  if (app === "documents" && route) {
    return generateDocumentsUrl(route, { includeScheme: true, params });
  }

  return generateAppUrl(app, { includeScheme: true, params });
}

/**
 * Generate URL for specific common actions
 */
export const generateCommonUrls = {
  /**
   * Generate URL to view a specific document
   */
  viewDocument(documentId: number): string {
    return generateDocumentsUrl({ type: "document-detail", id: documentId });
  },

  /**
   * Generate URL to create a new document
   */
  newDocument(contentType: "document" | "link" | "file" = "document"): string {
    return generateDocumentsUrl({ type: "publish", contentType });
  },

  /**
   * Generate URL to reply to a document
   */
  replyToDocument(
    replyTo: string,
    contentType: "document" | "link" | "file" = "document"
  ): string {
    return generateDocumentsUrl({ type: "publish", contentType, replyTo });
  },

  /**
   * Generate URL to edit a draft
   */
  editDraft(draftId: string): string {
    return generateDocumentsUrl({ type: "publish", editingDraftId: draftId });
  },

  /**
   * Generate URL to view drafts
   */
  viewDrafts(): string {
    return generateDocumentsUrl({ type: "drafts" });
  },

  /**
   * Generate URL to view documents list
   */
  viewDocuments(): string {
    return generateDocumentsUrl({ type: "documents-list" });
  },

  /**
   * Generate URL for pod collection
   */
  podCollection(): string {
    return generateAppUrl("pod-collection");
  },

  /**
   * Generate URL for pod editor
   */
  podEditor(): string {
    return generateAppUrl("pod-editor");
  },

  /**
   * Generate URL for frog crypto
   */
  frogCrypto(): string {
    return generateAppUrl("frogcrypto");
  }
};

/**
 * Utility to validate and normalize a generated URL
 */
export function validateGeneratedUrl(url: string): {
  valid: boolean;
  normalizedUrl: string;
  error?: string;
} {
  try {
    // Check if it starts with podnet://
    if (!url.startsWith("podnet://")) {
      return {
        valid: false,
        normalizedUrl: url,
        error: "URL must start with podnet://"
      };
    }

    // Try to parse as URL to validate structure
    const urlObj = new URL(url);

    // Reconstruct to ensure proper encoding
    const normalizedUrl = urlObj.toString();

    return {
      valid: true,
      normalizedUrl
    };
  } catch (error) {
    return {
      valid: false,
      normalizedUrl: url,
      error: `Invalid URL format: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}
