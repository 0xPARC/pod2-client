import { ThreadedReply as ThreadedReplyType } from "../../lib/replyUtils";
import { formatDateCompact } from "../../lib/dateUtils";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";

interface ThreadedReplyProps {
  reply: ThreadedReplyType;
  documentId: number;
  currentDocumentPostId: number;
  onNavigateToDocument?: (documentId: number) => void;
}

export function ThreadedReply({
  reply,
  documentId,
  currentDocumentPostId,
  onNavigateToDocument
}: ThreadedReplyProps) {
  const isReplyToCurrentDoc = reply.reply_to?.document_id === documentId;
  const isReplyToCurrentPost =
    reply.reply_to?.post_id === currentDocumentPostId;
  const maxDepth = 5; // Limit nesting depth for readability
  const displayDepth = Math.min(reply.depth, maxDepth);

  // Recursive component to render a reply and its children
  const renderThreadedReply = (reply: ThreadedReplyType): React.JSX.Element => {
    return (
      <div key={reply.id} className="space-y-4">
        <div
          className="border-l-2 border-muted pl-4"
          style={{
            marginLeft: displayDepth > 0 ? `${displayDepth * 16}px` : "0"
          }}
        >
          <div className="flex items-start justify-between mb-2">
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span className="font-medium text-blue-600">
                {reply.uploader_id}
              </span>
              <span>•</span>
              <span>{formatDateCompact(reply.created_at)}</span>
              <span>•</span>
              <span className="text-orange-600">
                #{reply.id} ({reply.upvote_count} upvotes)
              </span>
              {displayDepth > 0 && (
                <>
                  <span>•</span>
                  <span className="text-xs text-muted-foreground bg-muted px-2 py-1 rounded">
                    Level {displayDepth}
                  </span>
                </>
              )}
            </div>
          </div>

          {/* Show which version this reply targets */}
          {reply.reply_to && (
            <div className="mb-2">
              {isReplyToCurrentDoc ? (
                <Badge
                  variant="outline"
                  className="text-xs bg-green-50 text-green-700 border-green-200"
                >
                  Reply to this version
                </Badge>
              ) : isReplyToCurrentPost ? (
                <div className="flex items-center gap-2">
                  <Badge
                    variant="outline"
                    className="text-xs bg-yellow-50 text-yellow-700 border-yellow-200"
                  >
                    Reply to different version
                  </Badge>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      onNavigateToDocument?.(reply.reply_to!.document_id)
                    }
                    className="text-yellow-700 hover:text-yellow-900 p-0 h-auto font-normal text-xs underline"
                    disabled={!onNavigateToDocument}
                  >
                    (view version #{reply.reply_to.document_id})
                  </Button>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  <Badge
                    variant="outline"
                    className="text-xs bg-blue-50 text-blue-700 border-blue-200"
                  >
                    Reply to #{reply.reply_to.document_id}
                  </Badge>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      onNavigateToDocument?.(reply.reply_to!.document_id)
                    }
                    className="text-blue-700 hover:text-blue-900 p-0 h-auto font-normal text-xs underline"
                    disabled={!onNavigateToDocument}
                  >
                    (view doc #{reply.reply_to.document_id})
                  </Button>
                </div>
              )}
            </div>
          )}

          <h4 className="font-medium text-foreground mb-2">{reply.title}</h4>

          {reply.tags.length > 0 && (
            <div className="flex gap-1 mb-2">
              {reply.tags.map((tag, index) => (
                <Badge key={index} variant="outline" className="text-xs">
                  {tag}
                </Badge>
              ))}
            </div>
          )}

          {reply.authors.length > 0 && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground mb-2">
              <span>Authors:</span>
              {reply.authors.map((author, index) => (
                <Badge key={index} variant="secondary" className="text-xs">
                  {author}
                </Badge>
              ))}
            </div>
          )}

          <Button
            variant="ghost"
            size="sm"
            onClick={() => onNavigateToDocument?.(reply.id!)}
            className="text-blue-600 hover:text-blue-800 p-0 h-auto font-normal"
            disabled={!onNavigateToDocument}
          >
            View full reply →
          </Button>
        </div>

        {/* Render children recursively */}
        {reply.children.length > 0 && (
          <div className="space-y-4">
            {reply.children.map((child) => (
              <ThreadedReply
                key={child.id}
                reply={child}
                documentId={documentId}
                currentDocumentPostId={currentDocumentPostId}
                onNavigateToDocument={onNavigateToDocument}
              />
            ))}
          </div>
        )}
      </div>
    );
  };

  return renderThreadedReply(reply);
}
