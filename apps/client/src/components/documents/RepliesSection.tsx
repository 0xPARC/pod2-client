import { AlertCircleIcon, MessageSquareIcon } from "lucide-react";
import { DocumentMetadata } from "../../lib/documentApi";
import { buildReplyTree } from "../../lib/replyUtils";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { ThreadedReply } from "./ThreadedReply";

interface RepliesSectionProps {
  replies: DocumentMetadata[];
  repliesLoading: boolean;
  repliesError: string | null;
  documentId: number;
  postId: number;
  onNavigateToDocument?: (documentId: number) => void;
}

export function RepliesSection({
  replies,
  repliesLoading,
  repliesError,
  documentId,
  postId,
  onNavigateToDocument
}: RepliesSectionProps) {
  const replyTree = buildReplyTree(replies);

  return (
    <Card className="mb-8">
      <CardHeader>
        <CardTitle className="text-lg flex items-center gap-2">
          <MessageSquareIcon className="h-5 w-5" />
          Replies to Post #{postId} ({replies.length})
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

        {!repliesLoading && !repliesError && replies.length === 0 && (
          <div className="text-center py-8 text-muted-foreground">
            <MessageSquareIcon className="h-12 w-12 mx-auto mb-2 opacity-50" />
            <p>No replies yet</p>
          </div>
        )}

        {!repliesLoading && !repliesError && replies.length > 0 && (
          <div className="space-y-4">
            {replyTree.map((reply) => (
              <ThreadedReply
                key={reply.id}
                reply={reply}
                documentId={documentId}
                currentDocumentPostId={postId}
                onNavigateToDocument={onNavigateToDocument}
              />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
