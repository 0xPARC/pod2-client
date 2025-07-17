import {
  AlertCircleIcon,
  ClockIcon,
  FileTextIcon,
  PlusIcon,
  RefreshCwIcon
} from "lucide-react";
import { useEffect, useState } from "react";
import { DocumentMetadata, fetchDocuments } from "../../lib/documentApi";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { DocumentDetailView } from "./DocumentDetailView";
import { PublishDemoPage } from "./PublishDemoPage";

export function DocumentsView() {
  const [documents, setDocuments] = useState<DocumentMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedDocumentId, setSelectedDocumentId] = useState<number | null>(
    null
  );
  const [showPublishForm, setShowPublishForm] = useState(false);

  const loadDocuments = async () => {
    try {
      setLoading(true);
      setError(null);
      const docs = await fetchDocuments();
      setDocuments(docs);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load documents");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadDocuments();
  }, []);

  const formatDate = (dateString?: string) => {
    if (!dateString) return "Unknown";
    return new Date(dateString).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit"
    });
  };

  // If publish form is shown, show the publish page
  if (showPublishForm) {
    return (
      <PublishDemoPage 
        onBack={() => setShowPublishForm(false)}
        onPublishSuccess={(documentId) => {
          console.log("Document published with ID:", documentId);
          setShowPublishForm(false);
          loadDocuments(); // Refresh the document list
        }}
      />
    );
  }

  // If a document is selected, show the detail view
  if (selectedDocumentId !== null) {
    return (
      <DocumentDetailView
        documentId={selectedDocumentId}
        onBack={() => setSelectedDocumentId(null)}
      />
    );
  }

  return (
    <div className="p-6 min-h-screen w-full overflow-y-auto">
      <div className="w-full">
        <div className="mb-6 flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold mb-2">Documents</h1>
            <p className="text-muted-foreground">
              Documents from the PodNet server with cryptographic verification.
            </p>
          </div>
          <div className="flex gap-2">
            <Button 
              onClick={() => setShowPublishForm(true)} 
              className="bg-primary hover:bg-primary/90"
            >
              <PlusIcon className="h-4 w-4 mr-2" />
              Publish Document
            </Button>
            <Button onClick={loadDocuments} disabled={loading} variant="outline">
              <RefreshCwIcon
                className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`}
              />
              Refresh
            </Button>
          </div>
        </div>

        {error && (
          <Card className="mb-6 border-destructive">
            <CardContent className="pt-6">
              <div className="flex items-center gap-2 text-destructive">
                <AlertCircleIcon className="h-5 w-5" />
                <span>{error}</span>
              </div>
            </CardContent>
          </Card>
        )}

        {loading ? (
          <Card>
            <CardContent className="pt-6">
              <div className="flex items-center justify-center py-8">
                <RefreshCwIcon className="h-6 w-6 animate-spin mr-2" />
                Loading documents...
              </div>
            </CardContent>
          </Card>
        ) : documents.length === 0 ? (
          <Card>
            <CardContent className="pt-6">
              <div className="text-center py-8">
                <FileTextIcon className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
                <p className="text-muted-foreground">No documents found</p>
              </div>
            </CardContent>
          </Card>
        ) : (
          <div className="grid gap-4">
            {documents.map((doc) => (
              <Card
                key={doc.id}
                className="hover:shadow-md transition-shadow cursor-pointer"
                onClick={() => setSelectedDocumentId(doc.id!)}
              >
                <CardHeader>
                  <div className="flex items-start justify-between">
                    <div>
                      <CardTitle className="flex items-center gap-2">
                        <FileTextIcon className="h-5 w-5" />
                        Document #{doc.id}
                      </CardTitle>
                      <p className="text-sm text-muted-foreground mt-1">
                        Post {doc.post_id} • Revision {doc.revision}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      {doc.upvote_count > 0 && (
                        <Badge variant="secondary">
                          {doc.upvote_count} upvote
                          {doc.upvote_count !== 1 ? "s" : ""}
                        </Badge>
                      )}
                    </div>
                  </div>
                </CardHeader>
                <CardContent>
                  <div className="space-y-3">
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
                      <div>
                        <span className="font-medium">Uploader:</span>{" "}
                        {doc.uploader_id}
                      </div>
                      <div className="flex items-center gap-1">
                        <ClockIcon className="h-3 w-3" />
                        <span className="font-medium">Created:</span>{" "}
                        {formatDate(doc.created_at)}
                      </div>
                      <div>
                        <span className="font-medium">Content ID:</span>
                        <code className="ml-1 text-xs bg-muted px-1 py-0.5 rounded">
                          {doc.content_id.slice(0, 16)}...
                        </code>
                      </div>
                      {doc.reply_to && (
                        <div>
                          <span className="font-medium">Reply to:</span>{" "}
                          Document #{doc.reply_to}
                        </div>
                      )}
                    </div>

                    <div className="flex gap-2">
                      <span className="font-medium">Upvotes:</span>
                      <Badge variant="secondary" className="bg-muted">
                        {doc.upvote_count}
                      </Badge>
                    </div>

                    {doc.tags.length > 0 && (
                      <div>
                        <span className="font-medium text-sm">Tags:</span>
                        <div className="flex flex-wrap gap-1 mt-1">
                          {doc.tags.map((tag, index) => (
                            <Badge
                              key={index}
                              variant="outline"
                              className="text-xs"
                            >
                              {tag}
                            </Badge>
                          ))}
                        </div>
                      </div>
                    )}

                    {doc.authors.length > 0 && (
                      <div>
                        <span className="font-medium text-sm">Authors:</span>
                        <div className="flex flex-wrap gap-1 mt-1">
                          {doc.authors.map((author, index) => (
                            <Badge
                              key={index}
                              variant="secondary"
                              className="text-xs"
                            >
                              {author}
                            </Badge>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
