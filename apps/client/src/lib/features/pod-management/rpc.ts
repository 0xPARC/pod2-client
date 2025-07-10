import { invokeCommand } from "@/lib/rpc";
import type { MainPod, SignedPod, PodInfo } from "@pod2/pod2js";

// Re-export types for convenience
export type { PodInfo };

/**
 * Space/Folder information
 */
export interface SpaceInfo {
  id: string;
  created_at: string;
}

/**
 * Statistics about PODs in the application
 */
export interface PodStats {
  total_pods: number;
  signed_pods: number;
  main_pods: number;
}

/**
 * Lists of PODs organized by type
 */
export interface PodLists {
  signed_pods: PodInfo[];
  main_pods: PodInfo[];
}

/**
 * Complete application state data
 */
export interface AppStateData {
  pod_stats: PodStats;
  pod_lists: PodLists;
}

/**
 * Type guard for AppStateData
 */
function isAppStateData(obj: any): obj is AppStateData {
  return (
    obj &&
    typeof obj === "object" &&
    obj.pod_stats &&
    typeof obj.pod_stats.total_pods === "number" &&
    typeof obj.pod_stats.signed_pods === "number" &&
    typeof obj.pod_stats.main_pods === "number" &&
    obj.pod_lists &&
    Array.isArray(obj.pod_lists.signed_pods) &&
    Array.isArray(obj.pod_lists.main_pods)
  );
}

// =============================================================================
// Pod Management Operations
// =============================================================================

/**
 * Import a POD into the application
 * @param pod - The POD to import (SignedPod or MainPod)
 * @param label - Optional label for the POD
 */
export async function importPod(
  pod: SignedPod | MainPod,
  label?: string
): Promise<void> {
  const podType = pod.podType[1]; // Extract the string type from the tuple
  return invokeCommand("import_pod", {
    serializedPod: JSON.stringify(pod),
    podType,
    label
  });
}

/**
 * Set the pinned status of a POD
 * @param spaceId - The space/folder ID
 * @param podId - The POD ID
 * @param pinned - Whether the POD should be pinned
 */
export async function setPodPinned(
  spaceId: string,
  podId: string,
  pinned: boolean
): Promise<void> {
  return invokeCommand("set_pod_pinned", {
    spaceId,
    podId,
    pinned
  });
}

/**
 * List all spaces/folders
 * @returns Array of space information
 */
export async function listSpaces(): Promise<SpaceInfo[]> {
  return invokeCommand<SpaceInfo[]>("list_spaces");
}

/**
 * Insert ZuKYC sample pods to the default space
 * @returns Promise that resolves when pods are inserted
 */
export async function insertZuKycPods(): Promise<void> {
  return invokeCommand<void>("insert_zukyc_pods");
}

// =============================================================================
// State Management
// =============================================================================

/**
 * Get the current application state
 * @returns The application state data
 */
export async function getAppState(): Promise<AppStateData> {
  const result = await invokeCommand<any>("get_app_state");

  if (!isAppStateData(result)) {
    throw new Error("Invalid app state data received from backend");
  }

  return result;
}

/**
 * Trigger a state synchronization
 */
export async function triggerSync(): Promise<void> {
  return invokeCommand("trigger_sync");
}
