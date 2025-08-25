/**
 * URL parser for podnet:// deep-link scheme
 */

import type { MiniApp } from "../store";
import type {
  DeepLinkData,
  DeepLinkParams,
  ParseResult,
  DocumentsRouteData,
  SimpleRouteData,
  DocumentRoute
} from "./types";
import { VALID_APPS, DEFAULT_ROUTES } from "./types";

/**
 * Parse a deep-link URL into structured data
 *
 * Examples:
 * - podnet://documents/ → documents app, documents-list route
 * - podnet://documents/document/123 → documents app, document-detail route with id=123
 * - podnet://documents/publish?contentType=document → documents app, publish route with params
 * - podnet://pod-collection/ → pod-collection app
 */
export function parseDeepLinkUrl(url: string): ParseResult {
  try {
    // Normalize URL and create URL object
    const normalizedUrl = url.trim();
    if (!normalizedUrl.startsWith("podnet://")) {
      return {
        success: false,
        error: "URL must start with podnet://"
      };
    }

    const urlObj = new URL(normalizedUrl);

    // Extract app name from hostname
    const app = urlObj.hostname as MiniApp;
    if (!VALID_APPS.includes(app)) {
      return {
        success: false,
        error: `Invalid app name: ${app}. Valid apps: ${VALID_APPS.join(", ")}`
      };
    }

    // Parse query parameters
    const params: DeepLinkParams = {};
    urlObj.searchParams.forEach((value, key) => {
      params[key] = value;
    });

    // Parse app-specific route data
    const routeData = parseAppRoute(app, urlObj.pathname, params);

    const deepLinkData: DeepLinkData = {
      app,
      route: routeData,
      valid: true,
      originalUrl: url
    };

    return {
      success: true,
      data: deepLinkData
    };
  } catch (error) {
    return {
      success: false,
      error: `Failed to parse URL: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}

/**
 * Parse app-specific route from pathname and params
 */
function parseAppRoute(
  app: MiniApp,
  pathname: string,
  params: DeepLinkParams
): DocumentsRouteData | SimpleRouteData | undefined {
  switch (app) {
    case "documents":
      return parseDocumentsRoute(pathname, params);

    case "pod-collection":
    case "pod-editor":
    case "frogcrypto":
      return { app } as SimpleRouteData;

    default:
      return undefined;
  }
}

/**
 * Parse documents app routes from pathname and params
 */
function parseDocumentsRoute(
  pathname: string,
  params: DeepLinkParams
): DocumentsRouteData {
  const segments = pathname.split("/").filter(Boolean);

  // Handle different document routes
  if (segments.length === 0 || pathname === "/") {
    // podnet://documents/ → documents-list
    return {
      app: "documents",
      route: { type: "documents-list" }
    };
  }

  if (segments[0] === "document" && segments[1]) {
    // podnet://documents/document/123 → document-detail with id=123
    const id = parseInt(segments[1], 10);
    if (isNaN(id)) {
      throw new Error(`Invalid document ID: ${segments[1]}`);
    }

    return {
      app: "documents",
      route: { type: "document-detail", id }
    };
  }

  if (segments[0] === "drafts") {
    // podnet://documents/drafts → drafts
    return {
      app: "documents",
      route: { type: "drafts" }
    };
  }

  if (segments[0] === "publish") {
    // podnet://documents/publish?contentType=document&replyTo=post_123:456 → publish with params
    const route: DocumentRoute = { type: "publish" };

    // Parse optional parameters
    if (params.editingDraftId) {
      route.editingDraftId = params.editingDraftId;
    }

    if (params.contentType) {
      const contentType = params.contentType as "document" | "link" | "file";
      if (["document", "link", "file"].includes(contentType)) {
        route.contentType = contentType;
      }
    }

    if (params.replyTo) {
      route.replyTo = params.replyTo;
    }

    return {
      app: "documents",
      route
    };
  }

  if (segments[0] === "debug") {
    // podnet://documents/debug → debug
    return {
      app: "documents",
      route: { type: "debug" }
    };
  }

  // Unknown route, fallback to documents-list
  console.warn(
    `Unknown documents route: ${pathname}, falling back to documents-list`
  );
  return {
    app: "documents",
    route: { type: "documents-list" }
  };
}

/**
 * Create a fallback deep-link data for invalid URLs
 */
export function createFallbackDeepLink(
  originalUrl: string,
  app?: MiniApp
): DeepLinkData {
  const fallbackApp = app && VALID_APPS.includes(app) ? app : "documents";
  const fallbackRoute = DEFAULT_ROUTES[fallbackApp];

  return {
    app: fallbackApp,
    route: fallbackRoute
      ? ({ app: fallbackApp, route: fallbackRoute } as DocumentsRouteData)
      : undefined,
    valid: false,
    originalUrl
  };
}

/**
 * Validate a deep-link URL without fully parsing it
 */
export function isValidDeepLinkUrl(url: string): boolean {
  try {
    if (!url.startsWith("podnet://")) {
      return false;
    }

    const urlObj = new URL(url);
    const app = urlObj.hostname;

    return VALID_APPS.includes(app as MiniApp);
  } catch {
    return false;
  }
}
