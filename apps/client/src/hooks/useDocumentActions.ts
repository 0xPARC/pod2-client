import { useNavigate } from "@tanstack/react-router";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { toast } from "sonner";
import { formatReplyToId } from "../lib/contentUtils";
import {
  Document,
  DocumentVerificationResult,
  createDraft,
  deleteDocument,
  verifyDocumentPod,
  type DraftRequest
} from "../lib/documentApi";

export interface UseDocumentActionsReturn {
  isVerifying: boolean;
  verificationError: string | null;
  isDeleting: boolean;
  handleVerifyDocument: () => Promise<void>;
  handleDeleteDocument: () => Promise<void>;
  handleReplyToDocument: (selectedQuote?: string) => void;
  handleEditDocument: () => void;
  handleQuoteAndReply: (selectedText: string) => Promise<void>;
}

export const useDocumentActions = (
  currentDocument: Document | null,
  setVerificationResult: (result: DocumentVerificationResult | null) => void
): UseDocumentActionsReturn => {
  const [isVerifying, setIsVerifying] = useState(false);
  const [verificationError, setVerificationError] = useState<string | null>(
    null
  );
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
        navigate({ to: "/documents" });
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

    try {
      const draftRequest: DraftRequest = {
        title: `Re: ${currentDocument.metadata.title}`,
        content_type: "message", // Use "message" instead of "document"
        message: selectedQuote ?? "",
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
      navigate({
        to: "/documents/publish",
        search: { draftId, replyTo: replyToId }
      });
    } catch (error) {
      console.error("Failed to create draft with quote:", error);

      navigate({
        to: "/documents/publish",
        search: {
          replyTo: replyToId,
          title: "Re: " + currentDocument.metadata.title
        }
      });
    }
  };

  const navigate = useNavigate();

  const handleEditDocument = () => {
    if (!currentDocument) return;

    // Navigate directly to the edit route - all document data will be loaded by the route loader
    const documentId = currentDocument.metadata.id!;
    navigate({
      to: "/documents/document/$documentId/edit",
      params: { documentId: documentId.toString() }
    });
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
      navigate({
        to: "/documents/publish",
        search: { draftId, contentType: "document", replyTo: replyToId }
      });
    } catch (error) {
      console.error("Failed to create draft with quote:", error);
      toast.error("Failed to create quote draft");

      // Fallback to regular reply
      navigate({
        to: "/documents/publish",
        search: { contentType: "document", replyTo: replyToId }
      });
    }
  };

  return {
    isVerifying,
    verificationError,
    isDeleting,
    handleVerifyDocument,
    handleDeleteDocument,
    handleReplyToDocument,
    handleEditDocument,
    handleQuoteAndReply
  };
};
