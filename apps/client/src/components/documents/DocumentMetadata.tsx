import { Document } from "../../lib/documentApi";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";

interface DocumentMetadataProps {
  document: Document;
}

export function DocumentMetadata({ document }: DocumentMetadataProps) {
  return (
    <Card className="bg-muted/30">
      <CardHeader>
        <CardTitle className="text-lg text-muted-foreground">
          Document Metadata
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 text-sm">
          <div>
            <span className="font-medium text-muted-foreground">
              Document ID:
            </span>
            <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1">
              #{document.metadata.id}
            </div>
          </div>

          <div>
            <span className="font-medium text-muted-foreground">Post ID:</span>
            <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1">
              #{document.metadata.post_id}
            </div>
          </div>

          <div>
            <span className="font-medium text-muted-foreground">Revision:</span>
            <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1">
              r{document.metadata.revision}
            </div>
          </div>

          <div className="lg:col-span-2">
            <span className="font-medium text-muted-foreground">
              Content ID:
            </span>
            <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1 break-all">
              {document.metadata.content_id}
            </div>
          </div>

          <div>
            <span className="font-medium text-muted-foreground">
              Verified Upvotes:
            </span>
            <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1">
              {document.metadata.upvote_count}
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
