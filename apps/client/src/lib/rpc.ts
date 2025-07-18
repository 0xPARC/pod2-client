// =============================================================================
// Feature-based RPC API
// =============================================================================
//
// This file provides backward compatibility while delegating to the new
// feature-based organization. New code should import from specific features.

import { invoke } from "@tauri-apps/api/core";

// Import from feature modules
import * as podManagementFeature from "./features/pod-management";
import * as networkingFeature from "./features/networking";
import * as authoringFeature from "./features/authoring";
import * as integrationFeature from "./features/integration";

// =============================================================================
// Configuration Types
// =============================================================================

/**
 * Feature configuration for the application
 * This matches the Rust FeatureConfig struct
 */
export interface FeatureConfig {
  pod_management: boolean;
  networking: boolean;
  authoring: boolean;
  integration: boolean;
  frogcrypto: boolean;
}

// Re-export types from feature modules
export type {
  PodInfo,
  SpaceInfo,
  PodStats,
  PodLists,
  AppStateData
} from "./features/pod-management";

export type { PrivateKeyInfo } from "./features/authoring";

/**
 * Error type for RPC operations
 */
export interface RpcError {
  message: string;
  code?: string;
  details?: any;
}

/**
 * Result type for RPC operations
 */
export type RpcResult<T> = Promise<T>;

// =============================================================================
// Configuration Operations
// =============================================================================

/**
 * Get the current feature configuration from the backend
 * @returns The feature configuration loaded from environment variables
 */
export async function getFeatureConfig(): Promise<FeatureConfig> {
  try {
    return await invoke("get_feature_config_command");
  } catch (error) {
    console.error("Failed to get feature configuration:", error);
    // Return default configuration as fallback
    return {
      pod_management: true,
      networking: true,
      authoring: true,
      integration: true,
      frogcrypto: false
    };
  }
}

// =============================================================================
// Re-export functions for backward compatibility
// =============================================================================

// Pod Management operations
export const getAppState = podManagementFeature.getAppState;
export const triggerSync = podManagementFeature.triggerSync;
export const importPod = podManagementFeature.importPod;
export const deletePod = podManagementFeature.deletePod;
export const listSpaces = podManagementFeature.listSpaces;
export const insertZuKycPods = podManagementFeature.insertZuKycPods;

// Networking operations
export const startP2pNode = networkingFeature.startP2pNode;
export const sendPodToPeer = networkingFeature.sendPodToPeer;
export const sendMessageAsPod = networkingFeature.sendMessageAsPod;
export const getInboxMessages = networkingFeature.getInboxMessages;
export const acceptInboxMessage = networkingFeature.acceptInboxMessage;
export const getChats = networkingFeature.getChats;
export const getChatMessages = networkingFeature.getChatMessages;

// Authoring operations
export const signPod = authoringFeature.signPod;
export const getPrivateKeyInfo = authoringFeature.getPrivateKeyInfo;

// Integration operations
export const submitPodRequest = integrationFeature.submitPodRequest;

/**
 * Handle and format RPC errors consistently
 */
export function handleRpcError(error: any): never {
  if (typeof error === "string") {
    throw new Error(error);
  }
  if (error?.message) {
    throw new Error(error.message);
  }
  throw new Error("Unknown RPC error");
}

/**
 * Wrapper for Tauri invoke that handles errors consistently
 */
export async function invokeCommand<T>(
  command: string,
  args?: Record<string, any>
): Promise<T> {
  try {
    return await invoke(command, args);
  } catch (error) {
    handleRpcError(error);
  }
}

/**
 * Get a frog from the server
 * @returns Promise that resolves when the frog is retrieved
 */
export async function requestFrog(): RpcResult<void> {
  return invokeCommand<void>("request_frog");
}

// =============================================================================
// Exports for backward compatibility
// =============================================================================

// Re-export the existing functions with their original names
export { signPod as signPodLegacy, importPod as importPodLegacy };

// =============================================================================
// Feature modules for direct import
// =============================================================================

export {
  podManagementFeature,
  networkingFeature,
  authoringFeature,
  integrationFeature
};
