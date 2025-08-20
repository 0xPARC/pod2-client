import { useState, useEffect } from "react";
import {
  Document,
  DocumentReplyTree,
  DocumentVerificationResult,
  fetchDocument,
  fetchDocumentReplyTree,
  getCurrentUsername
} from "../lib/documentApi";

export interface UseDocumentDataReturn {
  currentDocument: Document | null;
  loading: boolean;
  error: string | null;
  verificationResult: DocumentVerificationResult | null;
  replyTree: DocumentReplyTree | null;
  repliesLoading: boolean;
  repliesError: string | null;
  currentUsername: string | null;
  upvoteCount: number;
  setUpvoteCount: (count: number) => void;
  setVerificationResult: (result: DocumentVerificationResult | null) => void;
  loadDocument: () => Promise<void>;
}

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
  const [replyTree, setReplyTree] = useState<DocumentReplyTree | null>(null);
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

      // Use new reply tree endpoint for single efficient API call
      const replyTreeData = await fetchDocumentReplyTree(documentId);
      setReplyTree(replyTreeData);
    } catch (err) {
      console.error("Failed to load reply tree:", err);
      setRepliesError(
        err instanceof Error ? err.message : "Failed to load replies"
      );
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
    replyTree,
    repliesLoading,
    repliesError,
    currentUsername,
    upvoteCount,
    setUpvoteCount,
    setVerificationResult,
    loadDocument
  };
};
