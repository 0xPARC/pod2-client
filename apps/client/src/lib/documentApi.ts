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
  reply_to?: number; // Document ID this replies to
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
export async function fetchDocuments(
  serverUrl?: string
): Promise<DocumentMetadata[]> {
  const actualServerUrl = serverUrl || (await getDocumentServerUrl());
  try {
    console.log(
      `[documentApi] Fetching documents from: ${actualServerUrl}/documents`
    );
    const response = await tauriFetch(`${actualServerUrl}/documents`);
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
export async function fetchDocument(
  id: number,
  serverUrl?: string
): Promise<Document> {
  const actualServerUrl = serverUrl || (await getDocumentServerUrl());
  const response = await tauriFetch(`${actualServerUrl}/documents/${id}`);
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
  id: number,
  serverUrl?: string
): Promise<DocumentMetadata[]> {
  const actualServerUrl = serverUrl || (await getDocumentServerUrl());
  const response = await tauriFetch(
    `${actualServerUrl}/documents/${id}/replies`
  );
  if (!response.ok) {
    throw new Error(
      `Failed to fetch replies for document ${id}: ${response.statusText}`
    );
  }
  return response.json();
}

/**
 * Fetch all posts with their documents from the PodNet server
 * @param serverUrl - Optional server URL (defaults to configuration value)
 * @returns Array of posts with documents
 */
export async function fetchPosts(serverUrl?: string): Promise<any[]> {
  const actualServerUrl = serverUrl || (await getDocumentServerUrl());
  const response = await tauriFetch(`${actualServerUrl}/posts`);
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
