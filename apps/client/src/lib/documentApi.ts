import { MainPod, SignedPod } from "@pod2/pod2js";
import { invoke } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

// =============================================================================
// Document Server API Types (PodNet)
// =============================================================================

/**
 * Document file attachment
 */
export interface DocumentFile {
  name: string; // Original filename
  content: number[]; // File bytes
  mime_type: string; // MIME type
}

/**
 * Document content supporting messages, files, and URLs
 */
export interface DocumentContent {
  message?: string; // Text message
  file?: DocumentFile; // File attachment
  url?: string; // URL reference
}

/**
 * Reply reference containing both post_id and document_id
 */
export interface ReplyReference {
  post_id: number;
  document_id: number;
}

/**
 * Document metadata from the PodNet server
 */
export interface DocumentMetadata {
  id?: number;
  content_id: string; // Content hash
  post_id: number;
  revision: number;
  created_at?: string;
  pod: MainPod; // MainPod proving document authenticity
  timestamp_pod: SignedPod; // Server timestamp pod
  uploader_id: string; // Username of uploader
  upvote_count: number; // Number of upvotes
  upvote_count_pod?: MainPod; // MainPod proving upvote count
  tags: string[]; // Tags for organization
  authors: string[]; // Authors for attribution
  reply_to?: ReplyReference; // Post and document IDs this replies to
  requested_post_id?: number; // Original post_id from request
  title: string; // Document title
}

/**
 * Complete document with metadata and content
 */
export interface Document {
  metadata: DocumentMetadata;
  content: DocumentContent;
}

/**
 * Verification result for a document
 */
export interface DocumentVerificationResult {
  publish_verified: boolean;
  timestamp_verified: boolean;
  upvote_count_verified: boolean;
  verification_details: Record<string, string>;
  verification_errors: string[];
}

// =============================================================================
// Draft Types
// =============================================================================

/**
 * Draft information from the database
 */
export interface DraftInfo {
  id: string; // UUID
  title: string;
  content_type: string; // "message", "file", or "url"
  message?: string;
  file_name?: string;
  file_content?: number[]; // File bytes as array
  file_mime_type?: string;
  url?: string;
  tags: string[]; // Parsed from JSON
  authors: string[]; // Parsed from JSON
  reply_to?: string; // Format: "post_id:document_id"
  session_id?: string;
  created_at: string;
  updated_at: string;
}

/**
 * Draft update/create request data
 */
export interface DraftRequest {
  title: string;
  content_type: string;
  message: string | null;
  file_name: string | null;
  file_content: number[] | null;
  file_mime_type: string | null;
  url: string | null;
  tags: string[];
  authors: string[];
  reply_to: string | null;
}

/**
 * Publish result from backend
 */
export interface PublishResult {
  success: boolean;
  document_id?: number;
  error_message?: string;
}

// =============================================================================
// Document Server API Client
// =============================================================================

// We'll get the server URL from configuration instead of hardcoding it

/**
 * Get the document server URL from configuration
 * @returns Promise resolving to the document server URL
 */
async function getDocumentServerUrl(): Promise<string> {
  const config = await invoke<any>("get_config_section", {
    section: "network"
  });
  return config.document_server;
}

/**
 * Fetch all documents from the PodNet server
 * @param serverUrl - Optional server URL (defaults to configuration value)
 * @returns Array of document metadata
 */
export async function fetchDocuments(): Promise<DocumentMetadata[]> {
  const serverUrl = await getDocumentServerUrl();
  try {
    console.log(
      `[documentApi] Fetching documents from: ${serverUrl}/documents`
    );
    const response = await tauriFetch(`${serverUrl}/documents`);
    console.log(
      `[documentApi] Response status: ${response.status} ${response.statusText}`
    );
    if (!response.ok) {
      throw new Error(`Failed to fetch documents: ${response.statusText}`);
    }
    return response.json();
  } catch (error) {
    console.error(`[documentApi] Error fetching documents:`, error);
    throw error;
  }
}

/**
 * Fetch a specific document by ID from the PodNet server
 * @param id - The document ID
 * @param serverUrl - Optional server URL (defaults to configuration value)
 * @returns Complete document with content
 */
export async function fetchDocument(id: number): Promise<Document> {
  const serverUrl = await getDocumentServerUrl();
  const response = await tauriFetch(`${serverUrl}/documents/${id}`);
  if (!response.ok) {
    throw new Error(`Failed to fetch document ${id}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * Fetch replies to a specific document
 * @param id - The document ID
 * @param serverUrl - Optional server URL (defaults to configuration value)
 * @returns Array of document metadata for replies
 */
export async function fetchDocumentReplies(
  id: number
): Promise<DocumentMetadata[]> {
  const serverUrl = await getDocumentServerUrl();
  const response = await tauriFetch(`${serverUrl}/documents/${id}/replies`);
  if (!response.ok) {
    throw new Error(
      `Failed to fetch replies for document ${id}: ${response.statusText}`
    );
  }
  return response.json();
}

/**
 * Fetch replies to all versions of a post
 * @param postId - The post ID
 * @param serverUrl - Optional server URL (defaults to localhost:3000)
 * @returns Array of document metadata for replies to any version of the post
 */
export async function fetchPostReplies(
  postId: number
): Promise<DocumentMetadata[]> {
  try {
    // Since there's no direct post replies endpoint, we'll fetch all documents
    // and filter for those that reply to any document in this post
    const allDocuments = await fetchDocuments();

    // Filter documents that have reply_to.post_id matching our postId
    const postReplies = allDocuments.filter(
      (doc) => doc.reply_to?.post_id === postId
    );

    // Sort by creation date (oldest first, like comment threads)
    return postReplies.sort((a, b) => {
      const dateA = new Date(a.created_at || 0).getTime();
      const dateB = new Date(b.created_at || 0).getTime();
      return dateA - dateB;
    });
  } catch (error) {
    throw new Error(
      `Failed to fetch replies for post ${postId}: ${error instanceof Error ? error.message : String(error)}`
    );
  }
}

/**
 * Fetch all posts with their documents from the PodNet server
 * @param serverUrl - Optional server URL (defaults to configuration value)
 * @returns Array of posts with documents
 */
export async function fetchPosts(): Promise<any[]> {
  const serverUrl = await getDocumentServerUrl();
  const response = await tauriFetch(`${serverUrl}/posts`);
  if (!response.ok) {
    throw new Error(`Failed to fetch posts: ${response.statusText}`);
  }
  return response.json();
}

/**
 * Verify a document's POD proofs using the Tauri backend
 * @param document - The complete document to verify
 * @returns Verification result with detailed status
 */
export async function verifyDocumentPod(
  document: Document
): Promise<DocumentVerificationResult> {
  try {
    console.log("Calling verifyDocumentPod with:", document);
    const result = await invoke<DocumentVerificationResult>(
      "verify_document_pod",
      {
        document: document
      }
    );
    return result;
  } catch (error) {
    console.error("Failed to verify document POD:", error);
    console.error("Document passed:", document);
    throw new Error(`Failed to verify document POD: ${error}`);
  }
}

// =============================================================================
// Draft API
// =============================================================================

/**
 * Create a new draft
 * @param request - Draft data to create
 * @returns Promise resolving to the draft ID (UUID)
 */
export async function createDraft(request: DraftRequest): Promise<string> {
  try {
    return await invoke<string>("create_draft", { request });
  } catch (error) {
    throw new Error(`Failed to create draft: ${error}`);
  }
}

/**
 * Update an existing draft
 * @param draftId - The draft ID (UUID) to update
 * @param request - Updated draft data
 * @returns Promise resolving to success status
 */
export async function updateDraft(
  draftId: string,
  request: DraftRequest
): Promise<boolean> {
  try {
    return await invoke<boolean>("update_draft", { draftId, request });
  } catch (error) {
    throw new Error(`Failed to update draft: ${error}`);
  }
}

/**
 * List all drafts
 * @returns Promise resolving to array of draft info
 */
export async function listDrafts(): Promise<DraftInfo[]> {
  try {
    return await invoke<DraftInfo[]>("list_drafts");
  } catch (error) {
    throw new Error(`Failed to list drafts: ${error}`);
  }
}

/**
 * Get a specific draft by ID
 * @param draftId - The draft ID (UUID) to retrieve
 * @returns Promise resolving to draft info or null if not found
 */
export async function getDraft(draftId: string): Promise<DraftInfo | null> {
  try {
    return await invoke<DraftInfo | null>("get_draft", { draftId });
  } catch (error) {
    throw new Error(`Failed to get draft: ${error}`);
  }
}

/**
 * Delete a draft by ID
 * @param draftId - The draft ID (UUID) to delete
 * @returns Promise resolving to success status
 */
export async function deleteDraft(draftId: string): Promise<boolean> {
  try {
    return await invoke<boolean>("delete_draft", { draftId });
  } catch (error) {
    throw new Error(`Failed to delete draft: ${error}`);
  }
}

/**
 * Publish a draft as a document
 * @param draftId - The draft ID (UUID) to publish
 * @param serverUrl - The document server URL
 * @returns Promise resolving to publish result
 */
export async function publishDraft(
  draftId: string,
  serverUrl: string
): Promise<PublishResult> {
  try {
    return await invoke<PublishResult>("publish_draft", { draftId, serverUrl });
  } catch (error) {
    throw new Error(`Failed to publish draft: ${error}`);
  }
}

/**
 * Delete result from backend
 */
export interface DeleteResult {
  success: boolean;
  document_id?: number;
  error_message?: string;
}

/**
 * Delete a document by ID
 * @param documentId - The document ID to delete
 * @param serverUrl - The document server URL
 * @returns Promise resolving to delete result
 */
export async function deleteDocument(
  documentId: number,
  serverUrl: string
): Promise<DeleteResult> {
  try {
    return await invoke<DeleteResult>("delete_document", {
      documentId,
      serverUrl
    });
  } catch (error) {
    throw new Error(`Failed to delete document: ${error}`);
  }
}

/**
 * Get current user's username
 * @returns Promise resolving to username or null if not set up
 */
export async function getCurrentUsername(): Promise<string | null> {
  try {
    return await invoke<string | null>("get_current_username");
  } catch (error) {
    console.error("Failed to get current username:", error);
    return null;
  }
}
