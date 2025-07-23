import { ArrowLeftIcon } from "lucide-react";
import { useAppStore } from "../../lib/store";
import { Button } from "../ui/button";
import { PublishForm } from "./PublishForm";

interface PublishPageProps {
  onBack?: () => void;
  onPublishSuccess?: (documentId: number) => void;
}

export function PublishPage({ onBack, onPublishSuccess }: PublishPageProps) {
  const { replyToDocumentId, setReplyToDocumentId } = useAppStore();
  
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
    if (onBack) {
      onBack();
    }
  };

  return (
    <div className="p-6 min-h-screen w-full overflow-y-auto">
      <div className="w-full max-w-4xl mx-auto">
        {onBack && (
          <Button variant="ghost" onClick={onBack} className="mb-4">
            <ArrowLeftIcon className="h-4 w-4 mr-2" />
            Back
          </Button>
        )}

        <div className="space-y-6">
          <div>
            <h1 className="text-3xl font-bold">
              {replyToDocumentId ? `Reply to Document #${replyToDocumentId.split(':')[1]} (Post ${replyToDocumentId.split(':')[0]})` : "Publish Document"}
            </h1>
            <p className="text-muted-foreground mt-2">
              {replyToDocumentId 
                ? `Create a reply to document #${replyToDocumentId.split(':')[1]} with cryptographic verification.`
                : "Create and publish a new document to the POD2 network with cryptographic verification."
              }
            </p>
          </div>

          <PublishForm
            onPublishSuccess={handlePublishSuccess}
            onCancel={handleCancel}
            replyTo={replyToDocumentId || undefined}
          />
        </div>
      </div>
    </div>
  );
}
