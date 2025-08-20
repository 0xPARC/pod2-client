import { AlertCircleIcon, MessageSquareIcon } from "lucide-react";
import { DocumentReplyTree } from "../../lib/documentApi";
import { DocumentReply } from "./DocumentReply";

interface RepliesSectionProps {
  replyTree: DocumentReplyTree | null;
  repliesLoading: boolean;
  repliesError: string | null;
  documentId: number;
  postId: number;
  onNavigateToDocument?: (documentId: number) => void;
  rootPostTitle: string;
}

export function RepliesSection({
  replyTree,
  repliesLoading,
  repliesError,
  documentId,
  postId,
  onNavigateToDocument,
  rootPostTitle
}: RepliesSectionProps) {
  const replyCount = replyTree?.replies.length || 0;

  return (
    <div className="mb-8">
      {/* Header */}
      <div className="mb-6">
        <h2 className="text-xl font-semibold flex items-center gap-2 mb-2">
          <MessageSquareIcon className="h-5 w-5" />
          Replies to Post #{postId} ({replyCount})
        </h2>
        <p className="text-sm text-muted-foreground">
          Showing replies to all versions of this post
        </p>
      </div>

      {/* Loading state */}
      {repliesLoading && (
        <div className="flex items-center justify-center py-8">
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary mr-2"></div>
          Loading replies...
        </div>
      )}

      {/* Error state */}
      {repliesError && (
        <div className="flex items-center gap-2 text-destructive py-4 bg-destructive/10 rounded-lg px-4">
          <AlertCircleIcon className="h-4 w-4" />
          <span>Failed to load replies: {repliesError}</span>
        </div>
      )}

      {/* Empty state */}
      {!repliesLoading && !repliesError && replyCount === 0 && (
        <div className="text-center py-12 text-muted-foreground bg-muted/30 rounded-lg">
          <MessageSquareIcon className="h-12 w-12 mx-auto mb-3 opacity-50" />
          <p className="text-lg">No replies yet</p>
          <p className="text-sm">Be the first to reply to this post</p>
        </div>
      )}

      {/* Replies */}
      {!repliesLoading && !repliesError && replyCount > 0 && (
        <div className="space-y-3">
          {replyTree!.replies.map((reply) => (
            <DocumentReply
              key={reply.document.id}
              replyTree={reply}
              documentId={documentId}
              currentDocumentPostId={postId}
              onNavigateToDocument={onNavigateToDocument}
              depth={0}
              rootPostTitle={rootPostTitle}
            />
          ))}
        </div>
      )}
    </div>
  );
}
