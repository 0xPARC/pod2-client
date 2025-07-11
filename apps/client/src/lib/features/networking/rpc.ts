// =============================================================================
// P2P Communications
// =============================================================================

import { invokeCommand } from "@/lib/rpc";

/**
 * Start the P2P node
 * @returns The node ID as a string
 */
export async function startP2pNode(): Promise<string> {
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
): Promise<void> {
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
): Promise<void> {
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
export async function getInboxMessages(): Promise<any[]> {
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
): Promise<string> {
  return invokeCommand<string>("accept_inbox_message", {
    messageId,
    chatAlias
  });
}

/**
 * Get all chats
 * @returns Array of chat information
 */
export async function getChats(): Promise<any[]> {
  return invokeCommand<any[]>("get_chats");
}

/**
 * Get messages for a specific chat
 * @param chatId - The chat ID
 * @returns Array of chat messages
 */
export async function getChatMessages(chatId: string): Promise<any[]> {
  return invokeCommand<any[]>("get_chat_messages", { chatId });
}
