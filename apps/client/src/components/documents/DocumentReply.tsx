import { useUpvote } from "@/hooks/useUpvote";
import { useLocation, useNavigate } from "@tanstack/react-router";
import { useEffect, useMemo, useRef, useState } from "react";
import { formatReplyToId } from "../../lib/contentUtils";
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
  owner: string;
}

export function DocumentReply({
  /* @ts-ignore */
  owner,
  replyTree,
  documentId,
  currentDocumentPostId,
  depth,
  rootPostTitle
}: DocumentReplyProps) {
  //const currentUsername = use(Route.useLoaderData().username);
  // const _isOwner = currentUsername && owner === currentUsername;

  const { document } = replyTree;
  const md = useMarkdownRenderer();
  const navigate = useNavigate();
  const location = useLocation();
  const [isCollapsed, setIsCollapsed] = useState(false);
  const replyRef = useRef<HTMLDivElement>(null);

  // Memoize rendered HTML for reply content
  const renderedMessageData = useMemo(() => {
    if (!replyTree.content?.message) return null;
    const { html, blocks } = renderMarkdownWithBlocks(
      md,
      replyTree.content.message
    );
    return { html, blocks };
  }, [replyTree.content?.message, md]);

  const isReplyToCurrentDoc = document.reply_to?.document_id === documentId;
  const isReplyToCurrentPost =
    document.reply_to?.post_id === currentDocumentPostId;

  // const handleEdit = useCallback(() => {
  //   navigate({
  //     to: "/documents/document/$documentId/edit",
  //     params: {
  //       documentId: Number(document.id).toString()
  //     }
  //   });
  // }, [document.id, navigate]);

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

  useEffect(() => {
    if (location.hash === `reply-${document.id}`) {
      if (replyRef.current) {
        replyRef.current.scrollIntoView({
          behavior: "smooth",
          block: "center"
        });
      }
    }
  }, [document.id, location.hash]);

  const { count, upvote, isPending } = useUpvote(
    documentId,
    document.upvote_count
  );

  return (
    <div className="space-y-3" ref={replyRef} id={`reply-${document.id}`}>
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
            <div className="flex items-center gap-1">
              <span
                className={`text-sm text-muted-foreground ${isCollapsed ? "hidden" : ""}`}
              >
                •
              </span>
              <button
                onClick={upvote}
                disabled={isPending}
                className={`transition-colors ${
                  isPending
                    ? "text-muted-foreground cursor-not-allowed"
                    : "text-muted-foreground hover:text-orange-500 cursor-pointer"
                }`}
                title="Upvote this document"
              >
                ▲
              </button>

              <span
                className={`text-sm text-muted-foreground ${isCollapsed ? "hidden" : ""}`}
              >
                {count} upvote
                {count !== 1 ? "s" : ""}
              </span>
            </div>
          </div>

          <div
            className={`flex items-center gap-2 ${isCollapsed ? "hidden" : ""}`}
          >
            {isReplyToCurrentPost && !isReplyToCurrentDoc && (
              <Badge variant="outline" className="text-xs">
                → This post
              </Badge>
            )}

            {/*
            TODO: Add edit button back in once document model is refactored
            {isOwner && (
              <Button
                variant="outline"
                size="sm"
                className="h-7 px-2 text-xs"
                onClick={handleEdit}
              >
                Edit
              </Button>
            )} */}

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
          {replyTree.content?.message && renderedMessageData && (
            <div className="mt-3">
              <div
                className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-lg prose-h2:text-base prose-h3:text-sm prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere prose-sm"
                dangerouslySetInnerHTML={{
                  __html: renderedMessageData.html
                }}
              />
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
                  owner={nestedReply.document.uploader_id}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
