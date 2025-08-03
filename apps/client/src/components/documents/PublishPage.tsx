import { useDocuments } from "../../lib/store";
import { PublishForm } from "./PublishForm";

interface PublishPageProps {
  onPublishSuccess?: (documentId: number) => void;
  editingDraftId?: string | null; // UUID
}

export function PublishPage({
  onPublishSuccess,
  editingDraftId
}: PublishPageProps) {
  const { replyToDocumentId, setReplyToDocumentId } = useDocuments();

  const handlePublishSuccess = (documentId: number) => {
    console.log("Document published successfully with ID:", documentId);
    // Clear the reply context after successful publish
    setReplyToDocumentId(null);
    if (onPublishSuccess) {
      onPublishSuccess(documentId);
    }
  };

  const handleCancel = () => {
    // Clear the reply context when canceling
    setReplyToDocumentId(null);
    // Navigation is now handled by the top-level back/forward buttons
  };

  return (
    <div className="p-6 min-h-screen w-full overflow-y-auto">
      <div className="w-full max-w-4xl mx-auto">
        <div className="space-y-6">
          <PublishForm
            onPublishSuccess={handlePublishSuccess}
            onCancel={handleCancel}
            replyTo={replyToDocumentId || undefined}
            editingDraftId={editingDraftId || undefined}
          />
        </div>
      </div>
    </div>
  );
}
