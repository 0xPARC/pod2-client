import { invokeCommand } from "@/lib/rpc";
import type { MainPod, PodInfo, SignedDict } from "@pod2/pod2js";

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
  spaces: SpaceInfo[];
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
    Array.isArray(obj.pod_lists.main_pods) &&
    obj.spaces &&
    Array.isArray(obj.spaces)
  );
}

// =============================================================================
// Pod Management Operations
// =============================================================================

/**
 * Import a signed dict into the application
 * @param signedDict - The signed dict to import
 * @param label - Optional label for the signed dict
 * @returns
 */
export async function importSignedDict(
  signedDict: SignedDict,
  label?: string
): Promise<void> {
  return invokeCommand("import_pod", {
    serializedPod: JSON.stringify(signedDict),
    podType: "Signed",
    label
  });
}

/**
 * Import a POD into the application
 * @param pod - The POD to import (MainPod)
 * @param label - Optional label for the POD
 */
export async function importPod(pod: MainPod, label?: string): Promise<void> {
  return invokeCommand("import_pod", {
    serializedPod: JSON.stringify(pod),
    podType: "Main",
    label
  });
}

/**
 * Import a POD from raw JSON content
 * @param serializedPod - JSON string containing the POD data
 * @param label - Optional label for the POD
 */
export async function importPodFromJson(
  serializedPod: string,
  label?: string
): Promise<void> {
  return invokeCommand("import_pod", {
    serializedPod,
    podType: "Main",
    label
  });
}

/**
 * Delete a POD from the database
 * @param spaceId - The space/folder ID
 * @param podId - The POD ID
 */
export async function deletePod(spaceId: string, podId: string): Promise<void> {
  return invokeCommand("delete_pod", {
    spaceId,
    podId
  });
}

/**
 * List all spaces/folders
 * @returns Array of space information
 */
export async function listSpaces(): Promise<SpaceInfo[]> {
  return invokeCommand<SpaceInfo[]>("list_spaces");
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
