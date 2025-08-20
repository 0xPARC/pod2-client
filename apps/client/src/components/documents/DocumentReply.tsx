import { DocumentReplyTree } from "../../lib/documentApi";
import { formatDateCompact } from "../../lib/dateUtils";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";

interface DocumentReplyProps {
  replyTree: DocumentReplyTree;
  documentId: number;
  currentDocumentPostId: number;
  onNavigateToDocument?: (documentId: number) => void;
  depth: number;
}

export function DocumentReply({
  replyTree,
  documentId,
  currentDocumentPostId,
  onNavigateToDocument,
  depth
}: DocumentReplyProps) {
  const { document } = replyTree;

  const isReplyToCurrentDoc = document.reply_to?.document_id === documentId;
  const isReplyToCurrentPost =
    document.reply_to?.post_id === currentDocumentPostId;
  const maxDepth = 5; // Limit nesting depth for readability
  const displayDepth = Math.min(depth, maxDepth);

  return (
    <div className="space-y-4">
      <div
        className="border-l-2 border-muted pl-4"
        style={{
          marginLeft: `${displayDepth * 16}px`
        }}
      >
        <div className="bg-muted/30 rounded-lg p-4 space-y-3">
          {/* Header */}
          <div className="flex items-center justify-between flex-wrap gap-2">
            <div className="flex items-center gap-2 flex-wrap">
              <Badge variant="outline" className="text-xs">
                Doc #{document.id}
              </Badge>
              <span className="text-sm text-muted-foreground">
                by {document.uploader_id}
              </span>
              <span className="text-sm text-muted-foreground">•</span>
              <span className="text-sm text-muted-foreground">
                {formatDateCompact(document.created_at)}
              </span>
              {document.upvote_count > 0 && (
                <>
                  <span className="text-sm text-muted-foreground">•</span>
                  <span className="text-sm text-muted-foreground">
                    {document.upvote_count} upvote
                    {document.upvote_count !== 1 ? "s" : ""}
                  </span>
                </>
              )}
            </div>

            <div className="flex items-center gap-2">
              {/* Reply relationship badges */}
              {isReplyToCurrentDoc && (
                <Badge variant="secondary" className="text-xs">
                  → This document
                </Badge>
              )}
              {isReplyToCurrentPost && !isReplyToCurrentDoc && (
                <Badge variant="outline" className="text-xs">
                  → This post
                </Badge>
              )}

              {onNavigateToDocument && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => onNavigateToDocument(document.id!)}
                  className="h-7 px-2 text-xs"
                >
                  View
                </Button>
              )}
            </div>
          </div>

          {/* Title */}
          <h4 className="font-medium text-sm leading-5">{document.title}</h4>

          {/* Content Preview */}
          {replyTree.content && (
            <div className="text-sm text-muted-foreground bg-muted/20 rounded p-3 border-l-2 border-muted">
              {replyTree.content.message && (
                <div className="line-clamp-3 whitespace-pre-wrap overflow-hidden">
                  {replyTree.content.message}
                </div>
              )}
              {replyTree.content.file && (
                <div className="flex items-center gap-2 text-xs">
                  📎{" "}
                  <span className="font-medium">
                    {replyTree.content.file.name}
                  </span>
                  <span>({replyTree.content.file.mime_type})</span>
                </div>
              )}
              {replyTree.content.url && (
                <div className="flex items-center gap-2 text-xs">
                  🔗 <span className="font-medium">URL:</span>
                  <span className="truncate">{replyTree.content.url}</span>
                </div>
              )}
            </div>
          )}

          {/* Tags */}
          {document.tags && document.tags.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {document.tags.map((tag) => (
                <Badge
                  key={tag}
                  variant="secondary"
                  className="text-xs px-2 py-0.5"
                >
                  {tag}
                </Badge>
              ))}
            </div>
          )}

          {/* Authors */}
          {document.authors && document.authors.length > 0 && (
            <div className="text-xs text-muted-foreground">
              Authors: {document.authors.join(", ")}
            </div>
          )}
        </div>
      </div>

      {/* Render nested replies recursively */}
      {replyTree.replies && replyTree.replies.length > 0 && (
        <div className="space-y-4">
          {replyTree.replies.map((nestedReply) => (
            <DocumentReply
              key={nestedReply.document.id}
              replyTree={nestedReply}
              documentId={documentId}
              currentDocumentPostId={currentDocumentPostId}
              onNavigateToDocument={onNavigateToDocument}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}
