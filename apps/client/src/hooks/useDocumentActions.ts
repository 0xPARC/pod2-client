import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import {
  Document,
  deleteDocument,
  verifyDocumentPod,
  DocumentVerificationResult,
  createDraft,
  type DraftRequest
} from "../lib/documentApi";
import { detectContentType, formatReplyToId } from "../lib/contentUtils";

export interface UseDocumentActionsReturn {
  isVerifying: boolean;
  verificationError: string | null;
  isUpvoting: boolean;
  isDeleting: boolean;
  handleVerifyDocument: () => Promise<void>;
  handleUpvote: () => Promise<void>;
  handleDeleteDocument: () => Promise<void>;
  handleReplyToDocument: (selectedQuote?: string) => void;
  handleEditDocument: () => void;
  handleQuoteAndReply: (selectedText: string) => Promise<void>;
}

export const useDocumentActions = (
  currentDocument: Document | null,
  setVerificationResult: (result: DocumentVerificationResult | null) => void,
  setUpvoteCount: (count: number) => void,
  navigateToPublish: (
    draftId?: string,
    contentType?: "document" | "link" | "file",
    replyTo?: string,
    editDocumentData?: any
  ) => void,
  navigateToDocumentsList: () => void
): UseDocumentActionsReturn => {
  const [isVerifying, setIsVerifying] = useState(false);
  const [verificationError, setVerificationError] = useState<string | null>(
    null
  );
  const [isUpvoting, setIsUpvoting] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);

  const handleVerifyDocument = async () => {
    if (!currentDocument) return;

    try {
      setIsVerifying(true);
      setVerificationError(null);
      const result = await verifyDocumentPod(currentDocument);
      console.log("Verification result:", result);
      setVerificationResult(result);
    } catch (err) {
      setVerificationError(
        err instanceof Error ? err.message : "Failed to verify document"
      );
    } finally {
      setIsVerifying(false);
    }
  };

  const handleUpvote = async () => {
    if (isUpvoting || !currentDocument) return;

    setIsUpvoting(true);

    // Show loading toast
    const loadingToast = toast("Generating upvote POD...", {
      duration: Infinity
    });

    try {
      const networkConfig = await invoke<any>("get_config_section", {
        section: "network"
      });
      const serverUrl = networkConfig.document_server;

      // Call the Tauri upvote command
      const result = await invoke<{
        success: boolean;
        new_upvote_count: number | null;
        error_message: string | null;
        already_upvoted: boolean;
      }>("upvote_document", {
        documentId: currentDocument.metadata.id,
        serverUrl: serverUrl
      });

      // Dismiss loading toast
      toast.dismiss(loadingToast);

      if (result.success && result.new_upvote_count !== null) {
        // Success - update count and show success message
        toast.success("Document upvoted successfully!");
        setUpvoteCount(result.new_upvote_count);
      } else if (result.already_upvoted) {
        // Already upvoted
        toast.info("You have already upvoted this document");
      } else {
        // Other error
        toast.error(result.error_message || "Failed to upvote document");
      }
    } catch (error) {
      // Dismiss loading toast and show error
      toast.dismiss(loadingToast);
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      toast.error(`Failed to upvote document: ${errorMessage}`);
      console.error("Upvote error:", error);
    } finally {
      setIsUpvoting(false);
    }
  };

  const handleDeleteDocument = async () => {
    if (!currentDocument || isDeleting) return;

    // Confirm deletion
    if (
      !confirm(
        "Are you sure you want to delete this document? This action cannot be undone."
      )
    ) {
      return;
    }

    setIsDeleting(true);

    // Show loading toast
    const loadingToast = toast("Deleting document...", {
      duration: Infinity
    });

    try {
      const networkConfig = await invoke<any>("get_config_section", {
        section: "network"
      });
      const serverUrl = networkConfig.document_server;

      // Call the Tauri delete command
      const result = await deleteDocument(
        currentDocument.metadata.id!,
        serverUrl
      );

      // Dismiss loading toast
      toast.dismiss(loadingToast);

      if (result.success) {
        toast.success("Document deleted successfully!");
        // Navigate back to documents list
        navigateToDocumentsList();
      } else {
        toast.error(result.error_message || "Failed to delete document");
      }
    } catch (error) {
      // Dismiss loading toast
      toast.dismiss(loadingToast);

      const errorMessage =
        error instanceof Error ? error.message : "Failed to delete document";
      console.error("Delete error:", error);
      toast.error(errorMessage);
    } finally {
      setIsDeleting(false);
    }
  };

  const handleReplyToDocument = async (selectedQuote?: string) => {
    if (!currentDocument) return;

    // Format: "post_id:document_id"
    const replyToId = formatReplyToId(
      currentDocument.metadata.post_id,
      currentDocument.metadata.id!
    );

    // If there's a selected quote, create a draft with the quote first
    if (selectedQuote) {
      try {
        const draftRequest: DraftRequest = {
          title: `Re: ${currentDocument.metadata.title}`,
          content_type: "message", // Use "message" instead of "document"
          message: selectedQuote,
          reply_to: replyToId,
          tags: [],
          authors: [],
          file_name: null,
          file_content: null,
          file_mime_type: null,
          url: null
        };

        const draftId = await createDraft(draftRequest);

        // Navigate to publish page with the draft
        navigateToPublish(draftId, undefined, replyToId);
      } catch (error) {
        console.error("Failed to create draft with quote:", error);
        toast.error("Failed to create draft with quote");
        // Fall back to normal reply without quote
        navigateToPublish(undefined, undefined, replyToId);
      }
    } else {
      // Navigate to publish page with reply context
      navigateToPublish(undefined, undefined, replyToId);
    }
  };

  const handleEditDocument = () => {
    if (!currentDocument) return;

    // Detect the correct content type based on document content
    const contentType = detectContentType(currentDocument);

    // Create the document data for editing
    const editDocumentData = {
      documentId: currentDocument.metadata.id!,
      postId: currentDocument.metadata.post_id,
      title: currentDocument.metadata.title || "",
      content: currentDocument.content,
      tags: currentDocument.metadata.tags,
      // For editing UI, flatten Author objects to usernames (github: github_username)
      authors: currentDocument.metadata.authors.map((a) =>
        (a as any).author_type === "github"
          ? (a as any).github_username
          : (a as any).username
      ),
      replyTo: currentDocument.metadata.reply_to
        ? formatReplyToId(
            currentDocument.metadata.reply_to.post_id,
            currentDocument.metadata.reply_to.document_id
          )
        : null
    };

    // Navigate to publish view in edit mode with route-specific data
    navigateToPublish(undefined, contentType, undefined, editDocumentData);
  };

  const handleQuoteAndReply = async (selectedText: string) => {
    if (!currentDocument) return;

    // Format the selected text as a markdown blockquote
    const quotedText = selectedText
      .split("\n")
      .map((line) => `> ${line}`)
      .join("\n");

    // Add attribution
    const attribution = `\n\n*— From "${currentDocument.metadata.title}" by ${currentDocument.metadata.uploader_id}*\n\n`;
    const fullQuote = quotedText + attribution;

    // Format: "post_id:document_id"
    const replyToId = formatReplyToId(
      currentDocument.metadata.post_id,
      currentDocument.metadata.id!
    );

    try {
      // Create a draft with the quoted content
      const draftRequest: DraftRequest = {
        title: `Re: ${currentDocument.metadata.title}`,
        content_type: "message" as const,
        message: fullQuote,
        file_name: null,
        file_content: null,
        file_mime_type: null,
        url: null,
        tags: [],
        authors: [],
        reply_to: replyToId
      };

      console.log("Creating draft with quote:", fullQuote);
      const draftId = await createDraft(draftRequest);

      // Navigate to publish page with the draft ID
      navigateToPublish(draftId, "document", replyToId);
    } catch (error) {
      console.error("Failed to create draft with quote:", error);
      toast.error("Failed to create quote draft");

      // Fallback to regular reply
      navigateToPublish(undefined, "document", replyToId);
    }
  };

  return {
    isVerifying,
    verificationError,
    isUpvoting,
    isDeleting,
    handleVerifyDocument,
    handleUpvote,
    handleDeleteDocument,
    handleReplyToDocument,
    handleEditDocument,
    handleQuoteAndReply
  };
};
