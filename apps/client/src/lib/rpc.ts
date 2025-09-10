// =============================================================================
// Feature-based RPC API
// =============================================================================
//
// This file provides backward compatibility while delegating to the new
// feature-based organization. New code should import from specific features.

import { invoke } from "@tauri-apps/api/core";

// Import from feature modules
import * as authoringFeature from "./features/authoring";
import * as podManagementFeature from "./features/pod-management";

// =============================================================================
// Build Information
// =============================================================================

/**
 * Get build information including git commit SHA
 * @returns Git commit SHA hash
 */
export async function getBuildInfo(): Promise<string> {
  return await invoke<string>("get_build_info");
}

// Re-export types from feature modules
export type {
  AppStateData,
  PodInfo,
  PodLists,
  PodStats,
  SpaceInfo
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
// Re-export functions for backward compatibility
// =============================================================================

// Pod Management operations
export const getAppState = podManagementFeature.getAppState;
export const triggerSync = podManagementFeature.triggerSync;
export const importPod = podManagementFeature.importPod;
export const deletePod = podManagementFeature.deletePod;
export const listSpaces = podManagementFeature.listSpaces;

// Authoring operations
export const signDict = authoringFeature.signDict;
export const getPrivateKeyInfo = authoringFeature.getPrivateKeyInfo;

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
export async function requestFrog(): RpcResult<number> {
  return invokeCommand<number>("request_frog");
}

export interface FrogPod {
  pod_id: string;
  data: FrogData | undefined;
}

export interface FrogData {
  frog_id: number;
  name: string;
  description: string;
  image_url: string;
  jump: number;
  speed: number;
  intelligence: number;
  beauty: number;
  temperament: number;
  rarity: number;
}

export async function listFrogs(): RpcResult<FrogPod[]> {
  return invokeCommand<FrogPod[]>("list_frogs");
}

export interface FrogedexEntry {
  frog_id: number;
  rarity: number;
  name: string;
  image_url: string;
  seen: boolean;
}

export async function getFrogedex(): RpcResult<FrogedexEntry[]> {
  return invokeCommand<FrogedexEntry[]>("get_frogedex");
}

export async function fixFrogDescriptions(): RpcResult<FrogPod[]> {
  return invokeCommand<FrogPod[]>("fix_frog_descriptions");
}

export interface ScoreResponse {
  score: number;
  timeout: number;
}

/**
 * Get the user's FrogCrypto score from the server
 * @returns Promise that resolves to the score
 */
export async function requestScore(): RpcResult<ScoreResponse> {
  return invokeCommand<ScoreResponse>("request_score");
}

export interface LeaderboardItem {
  username: string;
  score: number;
}

/**
 * Get the FrogCrypto leaderboard from the server
 * @returns Promise that resolves to the leaderboard
 */
export async function requestLeaderboard(): RpcResult<LeaderboardItem[]> {
  return invokeCommand<LeaderboardItem[]>("request_leaderboard");
}

// =============================================================================
// Exports for backward compatibility
// =============================================================================

// Re-export the existing functions with their original names
export { importPod as importPodLegacy, signDict as signPodLegacy };

// =============================================================================
// Feature modules for direct import
// =============================================================================

export { authoringFeature, podManagementFeature };
