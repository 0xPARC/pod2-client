/**
 * Deep-link URL validator with fallback handling
 */

import type { DeepLinkData, DocumentsRouteData } from "./types";
import {
  parseDeepLinkUrl,
  createFallbackDeepLink,
  isValidDeepLinkUrl
} from "./parser";
import { VALID_APPS } from "./types";

/**
 * Validation result with detailed information
 */
export interface ValidationResult {
  /** Whether the URL is valid */
  valid: boolean;
  /** Parsed deep-link data (with fallbacks applied if needed) */
  data: DeepLinkData;
  /** List of validation warnings */
  warnings: string[];
  /** List of validation errors */
  errors: string[];
}

/**
 * Validate a deep-link URL and return sanitized data with fallbacks
 * This is the main function to use for processing deep-link URLs
 */
export function validateDeepLinkUrl(url: string): ValidationResult {
  const warnings: string[] = [];
  const errors: string[] = [];

  // Quick validation first
  if (!isValidDeepLinkUrl(url)) {
    errors.push("Invalid deep-link URL format");
    return {
      valid: false,
      data: createFallbackDeepLink(url),
      warnings,
      errors
    };
  }

  // Parse the URL
  const parseResult = parseDeepLinkUrl(url);

  if (!parseResult.success) {
    errors.push(parseResult.error || "Failed to parse URL");
    return {
      valid: false,
      data: createFallbackDeepLink(url),
      warnings,
      errors
    };
  }

  const data = parseResult.data!;

  // Validate app-specific data
  const appValidation = validateAppSpecificData(data);
  warnings.push(...appValidation.warnings);
  errors.push(...appValidation.errors);

  // Apply fallbacks if needed
  const sanitizedData = applySafetyFallbacks(data, warnings);

  return {
    valid: errors.length === 0,
    data: sanitizedData,
    warnings,
    errors
  };
}

/**
 * Validate app-specific route data
 */
function validateAppSpecificData(data: DeepLinkData): {
  warnings: string[];
  errors: string[];
} {
  const warnings: string[] = [];
  const errors: string[] = [];

  switch (data.app) {
    case "documents":
      if (data.route && data.route.app === "documents") {
        const docValidation = validateDocumentsRoute(data.route);
        warnings.push(...docValidation.warnings);
        errors.push(...docValidation.errors);
      }
      break;

    case "pod-collection":
    case "pod-editor":
    case "frogcrypto":
      // These apps don't have complex validation requirements
      if (data.route && "route" in data.route && data.route.route) {
        warnings.push(`${data.app} doesn't support route parameters, ignoring`);
      }
      break;

    default:
      errors.push(`Unknown app: ${data.app}`);
  }

  return { warnings, errors };
}

/**
 * Validate documents-specific route data
 */
function validateDocumentsRoute(route: DocumentsRouteData): {
  warnings: string[];
  errors: string[];
} {
  const warnings: string[] = [];
  const errors: string[] = [];

  switch (route.route.type) {
    case "document-detail":
      if (!route.route.id || route.route.id <= 0) {
        errors.push("Document detail route requires a valid document ID");
      }
      break;

    case "publish":
      // Validate contentType if provided
      if (
        route.route.contentType &&
        !["document", "link", "file"].includes(route.route.contentType)
      ) {
        warnings.push(
          `Invalid contentType: ${route.route.contentType}, falling back to document`
        );
      }

      // Validate replyTo format if provided
      if (route.route.replyTo && !isValidReplyToFormat(route.route.replyTo)) {
        warnings.push(
          `Invalid replyTo format: ${route.route.replyTo}, should be post_<id>:<docId>`
        );
      }

      // Validate editingDraftId if provided (should be UUID format)
      if (
        route.route.editingDraftId &&
        !isValidUuid(route.route.editingDraftId)
      ) {
        warnings.push(
          `Invalid editingDraftId format: ${route.route.editingDraftId}, should be UUID`
        );
      }
      break;

    case "documents-list":
    case "drafts":
    case "debug":
      // These routes don't have additional validation requirements
      break;

    default:
      errors.push(`Unknown documents route type: ${(route.route as any).type}`);
  }

  return { warnings, errors };
}

/**
 * Apply safety fallbacks to ensure the data is always usable
 */
function applySafetyFallbacks(
  data: DeepLinkData,
  warnings: string[]
): DeepLinkData {
  // If app is invalid, fallback to documents
  if (!VALID_APPS.includes(data.app)) {
    warnings.push(`Invalid app ${data.app}, falling back to documents`);
    return {
      ...data,
      app: "documents",
      route: { app: "documents", route: { type: "documents-list" } },
      valid: false
    };
  }

  // If route data is invalid for documents app, provide fallback
  if (
    data.app === "documents" &&
    data.route &&
    data.route.app === "documents"
  ) {
    const route = data.route.route;

    // Fix invalid document IDs
    if (route.type === "document-detail" && (!route.id || route.id <= 0)) {
      warnings.push("Invalid document ID, falling back to documents list");
      return {
        ...data,
        route: { app: "documents", route: { type: "documents-list" } },
        valid: false
      };
    }

    // Fix invalid contentType in publish route
    if (
      route.type === "publish" &&
      route.contentType &&
      !["document", "link", "file"].includes(route.contentType)
    ) {
      return {
        ...data,
        route: {
          app: "documents",
          route: {
            ...route,
            contentType: "document"
          }
        }
      };
    }
  }

  return data;
}

/**
 * Check if a string is a valid UUID format
 */
function isValidUuid(str: string): boolean {
  const uuidRegex =
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
  return uuidRegex.test(str);
}

/**
 * Check if a replyTo string has valid format (post_<id>:<docId>)
 */
function isValidReplyToFormat(replyTo: string): boolean {
  const replyToRegex = /^post_\d+:\d+$/;
  return replyToRegex.test(replyTo);
}

/**
 * Get a safe deep-link data object, guaranteed to be navigable
 * This function never throws and always returns valid data
 */
export function getSafeDeepLinkData(url: string): DeepLinkData {
  try {
    const validation = validateDeepLinkUrl(url);
    return validation.data;
  } catch (error) {
    console.error("Error validating deep-link URL:", error);
    return createFallbackDeepLink(url);
  }
}

/**
 * Check if deep-link data represents a valid, navigable state
 */
export function isNavigableDeepLink(data: DeepLinkData): boolean {
  if (!VALID_APPS.includes(data.app)) {
    return false;
  }

  // Documents app requires valid route data
  if (data.app === "documents") {
    if (!data.route || data.route.app !== "documents") {
      return false;
    }

    const route = data.route.route;
    if (route.type === "document-detail" && (!route.id || route.id <= 0)) {
      return false;
    }
  }

  return true;
}
