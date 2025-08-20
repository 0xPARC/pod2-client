import { AlertCircleIcon, MessageSquareIcon } from "lucide-react";
import { DocumentReplyTree } from "../../lib/documentApi";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { DocumentReply } from "./DocumentReply";

interface RepliesSectionProps {
  replyTree: DocumentReplyTree | null;
  repliesLoading: boolean;
  repliesError: string | null;
  documentId: number;
  postId: number;
  onNavigateToDocument?: (documentId: number) => void;
}

export function RepliesSection({
  replyTree,
  repliesLoading,
  repliesError,
  documentId,
  postId,
  onNavigateToDocument
}: RepliesSectionProps) {
  const replyCount = replyTree?.replies.length || 0;

  return (
    <Card className="mb-8">
      <CardHeader>
        <CardTitle className="text-lg flex items-center gap-2">
          <MessageSquareIcon className="h-5 w-5" />
          Replies to Post #{postId} ({replyCount})
        </CardTitle>
        <p className="text-sm text-muted-foreground">
          Showing replies to all versions of this post
        </p>
      </CardHeader>
      <CardContent>
        {repliesLoading && (
          <div className="flex items-center justify-center py-8">
            <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary mr-2"></div>
            Loading replies...
          </div>
        )}

        {repliesError && (
          <div className="flex items-center gap-2 text-destructive py-4">
            <AlertCircleIcon className="h-4 w-4" />
            <span>Failed to load replies: {repliesError}</span>
          </div>
        )}

        {!repliesLoading && !repliesError && replyCount === 0 && (
          <div className="text-center py-8 text-muted-foreground">
            <MessageSquareIcon className="h-12 w-12 mx-auto mb-2 opacity-50" />
            <p>No replies yet</p>
          </div>
        )}

        {!repliesLoading && !repliesError && replyCount > 0 && (
          <div className="space-y-4">
            {replyTree!.replies.map((reply) => (
              <DocumentReply
                key={reply.document.id}
                replyTree={reply}
                documentId={documentId}
                currentDocumentPostId={postId}
                onNavigateToDocument={onNavigateToDocument}
                depth={0}
              />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
