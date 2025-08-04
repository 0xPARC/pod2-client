import { useDocuments } from "../../lib/store";
import { PublishDocumentForm } from "./forms/PublishDocumentForm";
import { PublishFileForm } from "./forms/PublishFileForm";
import { PublishLinkForm } from "./forms/PublishLinkForm";

interface PublishPageProps {
  onPublishSuccess?: (documentId: number) => void;
  editingDraftId?: string | null; // UUID - only used for documents
  contentType?: "document" | "link" | "file";
}

export function PublishPage({
  onPublishSuccess,
  editingDraftId,
  contentType = "document"
}: PublishPageProps) {
  const { replyToDocumentId, setReplyToDocumentId } = useDocuments();

  const handlePublishSuccess = (documentId: number) => {
    console.log("Content published successfully with ID:", documentId);
    // Clear the reply context after successful publish (only relevant for documents)
    if (contentType === "document") {
      setReplyToDocumentId(null);
    }
    if (onPublishSuccess) {
      onPublishSuccess(documentId);
    }
  };

  const handleCancel = () => {
    // Clear the reply context when canceling (only relevant for documents)
    if (contentType === "document") {
      setReplyToDocumentId(null);
    }
    // Navigation is now handled by the top-level back/forward buttons
  };

  const renderForm = () => {
    switch (contentType) {
      case "document":
        return (
          <PublishDocumentForm
            onPublishSuccess={handlePublishSuccess}
            onCancel={handleCancel}
            replyTo={replyToDocumentId || undefined}
            editingDraftId={editingDraftId || undefined}
          />
        );
      case "link":
        return (
          <PublishLinkForm
            onPublishSuccess={handlePublishSuccess}
            onCancel={handleCancel}
          />
        );
      case "file":
        return (
          <PublishFileForm
            onPublishSuccess={handlePublishSuccess}
            onCancel={handleCancel}
          />
        );
      default:
        return (
          <PublishDocumentForm
            onPublishSuccess={handlePublishSuccess}
            onCancel={handleCancel}
            replyTo={replyToDocumentId || undefined}
            editingDraftId={editingDraftId || undefined}
          />
        );
    }
  };

  return (
    <div className="w-full overflow-hidden h-full">
      {contentType === "document" ? (
        renderForm()
      ) : (
        <div className="p-6 min-h-screen w-full overflow-y-auto">
          <div className="w-full max-w-4xl mx-auto">
            <div className="space-y-6">{renderForm()}</div>
          </div>
        </div>
      )}
    </div>
  );
}
