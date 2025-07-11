import { MainPod, SignedPod } from "@pod2/pod2js";

// =============================================================================
// Document Server API Types (PodNet)
// =============================================================================

/**
 * Document file attachment
 */
export interface DocumentFile {
  name: string;      // Original filename
  content: number[]; // File bytes
  mime_type: string; // MIME type
}

/**
 * Document content supporting messages, files, and URLs
 */
export interface DocumentContent {
  message?: string;         // Text message
  file?: DocumentFile;      // File attachment
  url?: string;            // URL reference
}

/**
 * Document metadata from the PodNet server
 */
export interface DocumentMetadata {
  id?: number;
  content_id: string;       // Content hash
  post_id: number;
  revision: number;
  created_at?: string;
  pod: MainPod;            // MainPod proving document authenticity
  timestamp_pod: SignedPod; // Server timestamp pod
  uploader_id: string;      // Username of uploader
  upvote_count: number;     // Number of upvotes
  upvote_count_pod?: MainPod; // MainPod proving upvote count
  tags: string[];          // Tags for organization
  authors: string[];       // Authors for attribution
  reply_to?: number;       // Document ID this replies to
  requested_post_id?: number; // Original post_id from request
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
  isValid: boolean;
  publishVerified: boolean;
  timestampVerified: boolean;
  upvoteCountVerified: boolean;
  errors: string[];
}

// =============================================================================
// Document Server API Client
// =============================================================================

const DEFAULT_SERVER_URL = 'http://localhost:3000';

/**
 * Fetch all documents from the PodNet server
 * @param serverUrl - Optional server URL (defaults to localhost:3000)
 * @returns Array of document metadata
 */
export async function fetchDocuments(serverUrl: string = DEFAULT_SERVER_URL): Promise<DocumentMetadata[]> {
  const response = await fetch(`${serverUrl}/documents`);
  if (!response.ok) {
    throw new Error(`Failed to fetch documents: ${response.statusText}`);
  }
  return response.json();
}

/**
 * Fetch a specific document by ID from the PodNet server
 * @param id - The document ID
 * @param serverUrl - Optional server URL (defaults to localhost:3000)
 * @returns Complete document with content
 */
export async function fetchDocument(id: number, serverUrl: string = DEFAULT_SERVER_URL): Promise<Document> {
  const response = await fetch(`${serverUrl}/documents/${id}`);
  if (!response.ok) {
    throw new Error(`Failed to fetch document ${id}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * Fetch replies to a specific document
 * @param id - The document ID
 * @param serverUrl - Optional server URL (defaults to localhost:3000)
 * @returns Array of document metadata for replies
 */
export async function fetchDocumentReplies(id: number, serverUrl: string = DEFAULT_SERVER_URL): Promise<DocumentMetadata[]> {
  const response = await fetch(`${serverUrl}/documents/${id}/replies`);
  if (!response.ok) {
    throw new Error(`Failed to fetch replies for document ${id}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * Fetch all posts with their documents from the PodNet server
 * @param serverUrl - Optional server URL (defaults to localhost:3000)
 * @returns Array of posts with documents
 */
export async function fetchPosts(serverUrl: string = DEFAULT_SERVER_URL): Promise<any[]> {
  const response = await fetch(`${serverUrl}/posts`);
  if (!response.ok) {
    throw new Error(`Failed to fetch posts: ${response.statusText}`);
  }
  return response.json();
}