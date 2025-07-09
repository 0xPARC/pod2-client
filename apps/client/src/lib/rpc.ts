import { MainPod, SignedPod, Value, PodInfo } from "@pod2/pod2js";

// Re-export PodInfo for use in other files
export type { PodInfo };
import { invoke } from "@tauri-apps/api/core";

// =============================================================================
// App-Specific Types (complementing pod2js types)
// =============================================================================

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
// Type Guards and Validation
// =============================================================================

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

/**
 * Handle and format RPC errors consistently
 */
function handleRpcError(error: any): never {
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
async function invokeCommand<T>(
  command: string,
  args?: Record<string, any>
): Promise<T> {
  try {
    return await invoke(command, args);
  } catch (error) {
    handleRpcError(error);
  }
}

// =============================================================================
// Pod Operations
// =============================================================================

/**
 * Sign a POD with the given key-value pairs
 * @param values - The key-value pairs to include in the POD
 * @returns The signed POD
 */
export async function signPod(
  values: Record<string, Value>
): RpcResult<SignedPod> {
  const serializedPod = await invokeCommand<string>("sign_pod", {
    serializedPodValues: JSON.stringify(values)
  });
  return JSON.parse(serializedPod);
}

/**
 * Import a POD into the application
 * @param pod - The POD to import (SignedPod or MainPod)
 * @param label - Optional label for the POD
 */
export async function importPod(
  pod: SignedPod | MainPod,
  label?: string
): RpcResult<void> {
  const podType = pod.podType[1]; // Extract the string type from the tuple
  return invokeCommand("import_pod", {
    serializedPod: JSON.stringify(pod),
    podType,
    label
  });
}

/**
 * Submit a POD request and get back a MainPod proof
 * @param request - The POD request string
 * @returns The resulting MainPod
 */
export async function submitPodRequest(request: string): RpcResult<MainPod> {
  return invokeCommand<MainPod>("submit_pod_request", { request });
}

// =============================================================================
// P2P Communications
// =============================================================================

/**
 * Start the P2P node
 * @returns The node ID as a string
 */
export async function startP2pNode(): RpcResult<string> {
  return invokeCommand<string>("start_p2p_node");
}

/**
 * Send a POD to a peer
 * @param peerNodeId - The peer's node ID
 * @param podId - The ID of the POD to send
 * @param messageText - Optional message text
 * @param senderAlias - Optional sender alias
 */
export async function sendPodToPeer(
  peerNodeId: string,
  podId: string,
  messageText?: string,
  senderAlias?: string
): RpcResult<void> {
  return invokeCommand("send_pod_to_peer", {
    peerNodeId,
    podId,
    messageText,
    senderAlias
  });
}

/**
 * Send a message as a POD to a peer
 * @param peerNodeId - The peer's node ID
 * @param messageText - The message text
 * @param senderAlias - Optional sender alias
 */
export async function sendMessageAsPod(
  peerNodeId: string,
  messageText: string,
  senderAlias?: string
): RpcResult<void> {
  return invokeCommand("send_message_as_pod", {
    peerNodeId,
    messageText,
    senderAlias
  });
}

// =============================================================================
// Messaging
// =============================================================================

/**
 * Get inbox messages
 * @returns Array of inbox messages
 */
export async function getInboxMessages(): RpcResult<any[]> {
  return invokeCommand<any[]>("get_inbox_messages");
}

/**
 * Accept an inbox message
 * @param messageId - The message ID to accept
 * @param chatAlias - Optional chat alias
 * @returns The chat ID
 */
export async function acceptInboxMessage(
  messageId: string,
  chatAlias?: string
): RpcResult<string> {
  return invokeCommand<string>("accept_inbox_message", {
    messageId,
    chatAlias
  });
}

/**
 * Get all chats
 * @returns Array of chat information
 */
export async function getChats(): RpcResult<any[]> {
  return invokeCommand<any[]>("get_chats");
}

/**
 * Get messages for a specific chat
 * @param chatId - The chat ID
 * @returns Array of chat messages
 */
export async function getChatMessages(chatId: string): RpcResult<any[]> {
  return invokeCommand<any[]>("get_chat_messages", { chatId });
}

// =============================================================================
// Key Management
// =============================================================================

/**
 * Create a new private key
 * @param alias - Optional alias for the key
 * @param setAsDefault - Whether to set as default key
 * @returns The key ID
 */
export async function createPrivateKey(
  alias?: string,
  setAsDefault: boolean = false
): RpcResult<string> {
  return invokeCommand<string>("create_private_key", {
    alias,
    setAsDefault
  });
}

/**
 * List all private keys
 * @returns Array of private key information
 */
export async function listPrivateKeys(): RpcResult<any[]> {
  return invokeCommand<any[]>("list_private_keys");
}

// =============================================================================
// State Management
// =============================================================================

/**
 * Get the current application state
 * @returns The application state data
 */
export async function getAppState(): RpcResult<AppStateData> {
  const result = await invokeCommand<any>("get_app_state");

  if (!isAppStateData(result)) {
    throw new Error("Invalid app state data received from backend");
  }

  return result;
}

/**
 * Trigger a state synchronization
 */
export async function triggerSync(): RpcResult<void> {
  return invokeCommand("trigger_sync");
}

// =============================================================================
// Exports for backward compatibility
// =============================================================================

// Re-export the existing functions with their original names
export { signPod as signPodLegacy, importPod as importPodLegacy };
