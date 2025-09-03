import { queryClient, replyTreeQueryKey } from "@/lib/query";
import { useNavigate } from "@tanstack/react-router";
import { DocumentContent } from "../../lib/documentApi";
import { PublishDocumentForm } from "./forms/PublishDocumentForm";
import { PublishFileForm } from "./forms/PublishFileForm";
import { PublishLinkForm } from "./forms/PublishLinkForm";

export interface EditingDocument {
  documentId: number;
  postId: number;
  title: string;
  content: DocumentContent;
  tags: string[];
  authors: string[];
  replyTo?: string | null;
}

interface PublishPageProps {
  onPublishSuccess?: (documentId: number) => void;
  editingDraftId?: string | null; // UUID - only used for documents
  contentType?: "document" | "link" | "file";
  replyTo?: string;
  editingDocument?: EditingDocument; // For editing existing documents
}

export function PublishPage({
  onPublishSuccess,
  editingDraftId,
  contentType = "document",
  replyTo,
  editingDocument
}: PublishPageProps) {
  const navigate = useNavigate();

  const handlePublishSuccess = async (documentId: number) => {
    console.log("Content published successfully with ID:", documentId);
    console.log("editingDocument", editingDocument);
    if (editingDocument?.replyTo) {
      const [_, replyDocumentId] = editingDocument.replyTo.split(":");
      await queryClient.resetQueries({
        queryKey: replyTreeQueryKey(Number(replyDocumentId))
      });
      navigate({
        to: "/documents/document/$documentId",
        params: { documentId: replyDocumentId.toString() },
        search: { newReply: documentId.toString() },
        hash: "reply-" + documentId.toString(),
        hashScrollIntoView: true
      });
    } else if (replyTo) {
      const [_, replyDocumentId] = replyTo.split(":");
      await queryClient.resetQueries({
        queryKey: replyTreeQueryKey(Number(replyDocumentId))
      });
      navigate({
        to: "/documents/document/$documentId",
        params: { documentId: replyDocumentId.toString() },
        search: { newReply: documentId.toString() },
        hash: "reply-" + documentId.toString(),
        hashScrollIntoView: true
      });
    } else {
      // Navigate to the published document
      navigate({
        to: "/documents/document/$documentId",
        params: { documentId: documentId.toString() }
      });
    }

    if (onPublishSuccess) {
      onPublishSuccess(documentId);
    }
  };

  const renderForm = () => {
    switch (contentType) {
      case "document":
        return (
          <PublishDocumentForm
            onPublishSuccess={handlePublishSuccess}
            // Don't pass onCancel - let the form handle navigation internally
            replyTo={replyTo}
            editingDraftId={editingDraftId || undefined}
            editingDocument={editingDocument}
          />
        );
      case "link":
        return (
          <PublishLinkForm
            onPublishSuccess={handlePublishSuccess}
            // Don't pass onCancel - let the form handle navigation internally
            editingDocument={editingDocument}
          />
        );
      case "file":
        return (
          <PublishFileForm
            onPublishSuccess={handlePublishSuccess}
            // Don't pass onCancel - let the form handle navigation internally
            editingDocument={editingDocument}
          />
        );
      default:
        return (
          <PublishDocumentForm
            onPublishSuccess={handlePublishSuccess}
            // Don't pass onCancel - let the form handle navigation internally
            replyTo={replyTo}
            editingDraftId={editingDraftId || undefined}
            editingDocument={editingDocument}
          />
        );
    }
  };

  return (
    <div className="w-full overflow-hidden h-full">
      {contentType === "document" ? (
        renderForm()
      ) : (
        <div className="p-6 min-h-calc(100vh - var(--top-bar-height)) w-full overflow-y-auto">
          <div className="w-full max-w-4xl mx-auto">
            <div className="space-y-6">{renderForm()}</div>
          </div>
        </div>
      )}
    </div>
  );
}
