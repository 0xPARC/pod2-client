import { PublishDocumentForm } from "./forms/PublishDocumentForm";
import { PublishFileForm } from "./forms/PublishFileForm";
import { PublishLinkForm } from "./forms/PublishLinkForm";

interface PublishPageProps {
  onPublishSuccess?: (documentId: number) => void;
  editingDraftId?: string | null; // UUID - only used for documents
  contentType?: "document" | "link" | "file";
  replyTo?: string;
}

export function PublishPage({
  onPublishSuccess,
  editingDraftId,
  contentType = "document",
  replyTo
}: PublishPageProps) {
  const handlePublishSuccess = (documentId: number) => {
    console.log("Content published successfully with ID:", documentId);
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
          />
        );
      case "link":
        return (
          <PublishLinkForm
            onPublishSuccess={handlePublishSuccess}
            // Don't pass onCancel - let the form handle navigation internally
          />
        );
      case "file":
        return (
          <PublishFileForm
            onPublishSuccess={handlePublishSuccess}
            // Don't pass onCancel - let the form handle navigation internally
          />
        );
      default:
        return (
          <PublishDocumentForm
            onPublishSuccess={handlePublishSuccess}
            // Don't pass onCancel - let the form handle navigation internally
            replyTo={replyTo}
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
