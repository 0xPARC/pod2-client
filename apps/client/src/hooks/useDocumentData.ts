import { useState, useEffect } from "react";
import {
  Document,
  DocumentMetadata,
  DocumentVerificationResult,
  fetchDocument,
  fetchPostReplies,
  fetchDocumentReplies,
  getCurrentUsername
} from "../lib/documentApi";

export interface UseDocumentDataReturn {
  currentDocument: Document | null;
  loading: boolean;
  error: string | null;
  verificationResult: DocumentVerificationResult | null;
  replies: DocumentMetadata[];
  repliesLoading: boolean;
  repliesError: string | null;
  currentUsername: string | null;
  upvoteCount: number;
  setUpvoteCount: (count: number) => void;
  setVerificationResult: (result: DocumentVerificationResult | null) => void;
  loadDocument: () => Promise<void>;
}

// Recursively fetch all replies to build complete conversation tree
const fetchAllRepliesRecursively = async (
  postId: number,
  visited: Set<number> = new Set()
): Promise<DocumentMetadata[]> => {
  // Get direct replies to this post
  const directReplies = await fetchPostReplies(postId);
  const allReplies: DocumentMetadata[] = [...directReplies];

  // For each direct reply, recursively fetch its replies
  for (const reply of directReplies) {
    if (!reply.id || visited.has(reply.id)) {
      continue;
    }

    visited.add(reply.id);

    try {
      // Fetch replies to this specific document
      const nestedReplies = await fetchDocumentReplies(reply.id);

      if (nestedReplies.length > 0) {
        allReplies.push(...nestedReplies);

        // Recursively fetch replies to nested replies
        for (const nestedReply of nestedReplies) {
          if (nestedReply.id && !visited.has(nestedReply.id)) {
            visited.add(nestedReply.id);
            const deeperReplies = await fetchDocumentReplies(nestedReply.id);
            allReplies.push(...deeperReplies);
          }
        }
      }
    } catch (error) {
      console.warn(`Failed to fetch replies for document ${reply.id}:`, error);
      // Continue with other replies even if one fails
    }
  }

  return allReplies;
};

export const useDocumentData = (
  documentId: number,
  updateCurrentRouteTitle?: (title: string) => void
): UseDocumentDataReturn => {
  const [currentDocument, setCurrentDocument] = useState<Document | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [verificationResult, setVerificationResult] =
    useState<DocumentVerificationResult | null>(null);
  const [upvoteCount, setUpvoteCount] = useState<number>(0);
  const [replies, setReplies] = useState<DocumentMetadata[]>([]);
  const [repliesLoading, setRepliesLoading] = useState(false);
  const [repliesError, setRepliesError] = useState<string | null>(null);
  const [currentUsername, setCurrentUsername] = useState<string | null>(null);

  const loadDocument = async () => {
    try {
      setLoading(true);
      setError(null);
      const doc = await fetchDocument(documentId);
      setCurrentDocument(doc);
      setUpvoteCount(doc.metadata.upvote_count);

      // Update the route title with the document title
      if (doc.metadata.title && updateCurrentRouteTitle) {
        updateCurrentRouteTitle(doc.metadata.title);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load document");
    } finally {
      setLoading(false);
    }
  };

  const loadReplies = async () => {
    if (!documentId || !currentDocument) return;

    try {
      setRepliesLoading(true);
      setRepliesError(null);

      // Use recursive fetching to get complete conversation tree
      const allRepliesData = await fetchAllRepliesRecursively(
        currentDocument.metadata.post_id
      );
      setReplies(allRepliesData);
    } catch (err) {
      // Fallback to basic post replies if recursive fails
      try {
        console.warn(
          "Recursive replies failed, falling back to basic post replies:",
          err
        );
        const postRepliesData = await fetchPostReplies(
          currentDocument.metadata.post_id
        );
        setReplies(postRepliesData);
      } catch (fallbackErr) {
        console.error("Both recursive and basic replies failed:", fallbackErr);
        setRepliesError(
          fallbackErr instanceof Error
            ? fallbackErr.message
            : "Failed to load replies"
        );
      }
    } finally {
      setRepliesLoading(false);
    }
  };

  const loadCurrentUsername = async () => {
    try {
      const username = await getCurrentUsername();
      setCurrentUsername(username);
    } catch (error) {
      console.error("Failed to get current username:", error);
    }
  };

  useEffect(() => {
    loadDocument();
  }, [documentId]);

  useEffect(() => {
    loadCurrentUsername();
  }, []);

  useEffect(() => {
    if (currentDocument) {
      loadReplies();
    }
  }, [currentDocument]);

  return {
    currentDocument,
    loading,
    error,
    verificationResult,
    replies,
    repliesLoading,
    repliesError,
    currentUsername,
    upvoteCount,
    setUpvoteCount,
    setVerificationResult,
    loadDocument
  };
};
