import { useMemo, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { formatReplyToId, isMarkdownContent } from "../../lib/contentUtils";
import { formatDateCompact } from "../../lib/dateUtils";
import { DocumentReplyTree } from "../../lib/documentApi";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import {
  renderMarkdownWithBlocks,
  useMarkdownRenderer
} from "./markdownRenderer";

interface DocumentReplyProps {
  replyTree: DocumentReplyTree;
  documentId: number;
  currentDocumentPostId: number;
  depth: number;
  rootPostTitle: string;
}

export function DocumentReply({
  replyTree,
  documentId,
  currentDocumentPostId,
  depth,
  rootPostTitle
}: DocumentReplyProps) {
  const { document } = replyTree;
  const md = useMarkdownRenderer();
  const navigate = useNavigate();
  const [isCollapsed, setIsCollapsed] = useState(false);

  // Memoize rendered HTML for reply content
  const renderedMessageData = useMemo(() => {
    if (!replyTree.content?.message) return null;

    const isMarkdown = isMarkdownContent(replyTree.content.message);

    if (isMarkdown) {
      const { html, blocks } = renderMarkdownWithBlocks(
        md,
        replyTree.content.message
      );
      return { html, blocks, isMarkdown };
    }

    return { html: null, blocks: [], isMarkdown };
  }, [replyTree.content?.message, md]);

  const isReplyToCurrentDoc = document.reply_to?.document_id === documentId;
  const isReplyToCurrentPost =
    document.reply_to?.post_id === currentDocumentPostId;

  const handleReply = () => {
    const replyToId = formatReplyToId(document.post_id, document.id!);
    const replyTitle = generateReplyTitle();
    navigate({
      to: "/documents/publish",
      search: {
        contentType: "document",
        replyTo: replyToId,
        title: replyTitle
      }
    });
  };

  const toggleCollapsed = () => {
    setIsCollapsed(!isCollapsed);
  };

  // Generate reply title based on context
  const generateReplyTitle = (): string => {
    if (document.reply_to) {
      // This is a reply to a reply - use author name
      return `Reply to ${document.uploader_id} on ${rootPostTitle}`;
    } else {
      // This is a direct reply to the root post
      return `Reply to ${rootPostTitle}`;
    }
  };

  // Alternate background colors based on depth
  const bgClass = depth % 2 === 0 ? "bg-background" : "bg-muted/20";

  return (
    <div className="space-y-3">
      <div className={`border rounded-md p-3 space-y-3 ${bgClass}`}>
        {/* Header */}
        <div className="flex items-center justify-between flex-wrap gap-2">
          <div className="flex items-center gap-2 flex-wrap">
            <button
              onClick={toggleCollapsed}
              className="text-xs text-muted-foreground hover:text-foreground font-mono w-6 text-left"
              title={isCollapsed ? "Expand" : "Collapse"}
            >
              {isCollapsed ? "[+]" : "[-]"}
            </button>
            <span className="text-sm font-medium">{document.uploader_id}</span>
            <span className="text-sm text-muted-foreground">•</span>
            <span className="text-sm text-muted-foreground">
              {formatDateCompact(document.created_at)}
            </span>
            {document.upvote_count > 0 && (
              <>
                <span
                  className={`text-sm text-muted-foreground ${isCollapsed ? "hidden" : ""}`}
                >
                  •
                </span>
                <span
                  className={`text-sm text-muted-foreground ${isCollapsed ? "hidden" : ""}`}
                >
                  {document.upvote_count} upvote
                  {document.upvote_count !== 1 ? "s" : ""}
                </span>
              </>
            )}
          </div>

          <div
            className={`flex items-center gap-2 ${isCollapsed ? "hidden" : ""}`}
          >
            {isReplyToCurrentPost && !isReplyToCurrentDoc && (
              <Badge variant="outline" className="text-xs">
                → This post
              </Badge>
            )}

            <Button
              variant="outline"
              size="sm"
              onClick={handleReply}
              className="h-7 px-2 text-xs"
            >
              Reply
            </Button>
          </div>
        </div>

        {/* Expanded Content */}
        <div className={isCollapsed ? "hidden" : ""}>
          {/* Reply Content */}
          {replyTree.content?.message && (
            <div className="mt-3">
              {renderedMessageData?.isMarkdown ? (
                <div
                  className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-lg prose-h2:text-base prose-h3:text-sm prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere prose-sm"
                  dangerouslySetInnerHTML={{
                    __html: renderedMessageData.html!
                  }}
                />
              ) : (
                <div className="prose prose-neutral max-w-none dark:prose-invert prose-sm">
                  <p className="whitespace-pre-wrap">
                    {replyTree.content.message}
                  </p>
                </div>
              )}
            </div>
          )}

          {/* File and URL attachments */}
          {(replyTree.content?.file || replyTree.content?.url) && (
            <div className="mt-2 space-y-1">
              {replyTree.content.file && (
                <div className="flex items-center gap-2 text-xs text-muted-foreground bg-muted/10 rounded px-2 py-1">
                  📎{" "}
                  <span className="font-medium">
                    {replyTree.content.file.name}
                  </span>
                  <span>({replyTree.content.file.mime_type})</span>
                </div>
              )}
              {replyTree.content.url && (
                <div className="flex items-center gap-2 text-xs text-muted-foreground bg-muted/10 rounded px-2 py-1">
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

          {/* Render nested replies recursively */}
          {replyTree.replies && replyTree.replies.length > 0 && (
            <div className="space-y-3 mt-3">
              {replyTree.replies.map((nestedReply) => (
                <DocumentReply
                  key={nestedReply.document.id}
                  replyTree={nestedReply}
                  documentId={documentId}
                  currentDocumentPostId={currentDocumentPostId}
                  depth={depth + 1}
                  rootPostTitle={rootPostTitle}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
