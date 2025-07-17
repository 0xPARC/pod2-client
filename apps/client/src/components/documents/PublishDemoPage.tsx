import { ArrowLeftIcon } from "lucide-react";
import { Button } from "../ui/button";
import { PublishForm } from "./PublishForm";

interface PublishDemoPageProps {
  onBack?: () => void;
  onPublishSuccess?: (documentId: number) => void;
}

export function PublishDemoPage({ onBack, onPublishSuccess }: PublishDemoPageProps) {
  const handlePublishSuccess = (documentId: number) => {
    console.log("Document published successfully with ID:", documentId);
    if (onPublishSuccess) {
      onPublishSuccess(documentId);
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
            <h1 className="text-3xl font-bold">Publish Document</h1>
            <p className="text-muted-foreground mt-2">
              Create and publish a new document to the POD2 network with cryptographic verification.
            </p>
          </div>

          <PublishForm 
            onPublishSuccess={handlePublishSuccess}
            onCancel={onBack}
          />
        </div>
      </div>
    </div>
  );
}