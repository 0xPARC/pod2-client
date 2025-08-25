/**
 * Deep-link system types for podnet:// URL scheme
 */

import type { MiniApp } from "../store";

/**
 * Document route types for deep linking
 */
export type DocumentRoute =
  | { type: "documents-list" }
  | { type: "document-detail"; id: number }
  | {
      type: "publish";
      contentType?: string;
      replyTo?: string;
      title?: string;
      editingDraftId?: string;
    }
  | { type: "drafts" }
  | { type: "debug" };

/**
 * Base interface for parsed deep-link URLs
 */
export interface DeepLinkData {
  /** The mini-app to navigate to */
  app: MiniApp;
  /** App-specific route data */
  route?: AppRouteData;
  /** Whether the URL was valid and parseable */
  valid: boolean;
  /** Original URL that was parsed */
  originalUrl: string;
}

/**
 * Union type for app-specific route data
 */
export type AppRouteData = DocumentsRouteData | SimpleRouteData;

/**
 * Route data for the documents mini-app
 */
export interface DocumentsRouteData {
  app: "documents";
  route: DocumentRoute;
}

/**
 * Route data for simple mini-apps (pod-collection, pod-editor, frogcrypto)
 */
export interface SimpleRouteData {
  app: "pod-collection" | "pod-editor" | "frogcrypto";
  route?: never; // These apps don't have complex routing
}

/**
 * Parameters that can be passed in deep-link URLs
 */
export interface DeepLinkParams {
  [key: string]: string | undefined;
}

/**
 * Result of URL parsing operation
 */
export interface ParseResult {
  /** Whether parsing was successful */
  success: boolean;
  /** Parsed deep-link data (only present if success=true) */
  data?: DeepLinkData;
  /** Error message (only present if success=false) */
  error?: string;
}

/**
 * Configuration for URL generation
 */
export interface GenerateUrlOptions {
  /** Whether to include the podnet:// scheme (default: true) */
  includeScheme?: boolean;
  /** Additional query parameters to include */
  params?: DeepLinkParams;
}

/**
 * Deep-link event handler function type
 */
export type DeepLinkHandler = (data: DeepLinkData) => void;

/**
 * Predefined route patterns for each mini-app
 */
export const ROUTE_PATTERNS = {
  documents: {
    list: "/",
    detail: "/document/:id",
    drafts: "/drafts",
    publish: "/publish",
    debug: "/debug"
  },
  "pod-collection": {
    default: "/"
  },
  "pod-editor": {
    default: "/"
  },
  frogcrypto: {
    default: "/"
  }
} as const;

/**
 * Valid mini-app names for deep-linking
 */
export const VALID_APPS: readonly MiniApp[] = [
  "documents",
  "pod-collection",
  "pod-editor",
  "frogcrypto"
] as const;

/**
 * Default fallback routes for each app
 */
export const DEFAULT_ROUTES: Record<MiniApp, DocumentRoute | null> = {
  documents: { type: "documents-list" },
  "pod-collection": null,
  "pod-editor": null,
  frogcrypto: null
} as const;
