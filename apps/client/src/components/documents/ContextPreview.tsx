import {
  ChevronDownIcon,
  ChevronUpIcon,
  MessageSquareIcon,
  TextQuoteIcon
} from "lucide-react";
import { useState } from "react";
import { formatBlockQuotes, groupAdjacentBlocks } from "../../lib/blockUtils";
import type { Document } from "../../lib/documentApi";
import { useDocuments } from "../../lib/store";
import { Button } from "../ui/button";
import { DocumentContentWithSelection } from "./DocumentContentWithSelection";

interface ContextPreviewProps {
  document: Document;
  className?: string;
  onQuoteText?: (quotedText: string) => void; // Callback to insert quoted text
}

export function ContextPreview({
  document: currentDocument,
  className = "",
  onQuoteText
}: ContextPreviewProps) {
  const [isExpanded, setIsExpanded] = useState(true); // Start expanded

  const { metadata } = currentDocument;

  // Get context selection state from store
  const {
    contextSelectedBlockIndices,
    contextSelectedBlockTexts,
    clearContextBlockSelection
  } = useDocuments();

  const hasSelection = contextSelectedBlockIndices.length > 0;

  // Handle copying selected blocks to editor
  const handleCopyToEditor = () => {
    if (!onQuoteText || contextSelectedBlockIndices.length === 0) return;

    // Group adjacent blocks for better quote formatting
    const groups = groupAdjacentBlocks(contextSelectedBlockIndices);
    const groupedBlockTexts = groups.map((group) =>
      group.map((index) => {
        const textIndex = contextSelectedBlockIndices.indexOf(index);
        return contextSelectedBlockTexts[textIndex] || "";
      })
    );

    // Format as quotes
    let quotedText: string;
    if (groups.length === 1) {
      // Single group of blocks
      quotedText = formatBlockQuotes(groupedBlockTexts[0]);
    } else {
      // Multiple groups - format each group and join with just the separator
      // formatBlockQuotes already adds "\n\n" at the end, so we just need to trim and join
      quotedText =
        groupedBlockTexts
          .map((groupBlocks) => formatBlockQuotes(groupBlocks).trimEnd())
          .join("\n\n") + "\n\n";
    }

    onQuoteText(quotedText);
    clearContextBlockSelection();
  };

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

  return (
    <div
      className={`border-l-4 border-l-blue-200 bg-blue-50/30 dark:bg-blue-950/10 dark:border-l-blue-800 ${className}`}
    >
      {/* Fixed Header Bar - Always Visible */}
      <div className="flex items-center justify-between px-4 py-3 border-b bg-blue-50/50 dark:bg-blue-950/20">
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <MessageSquareIcon className="h-4 w-4 text-blue-600 shrink-0" />
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-accent-foreground line-clamp-1">
              Replying to: {metadata.title || "Untitled"}
            </div>
            <div className="flex items-center gap-2 text-xs text-muted-foreground mt-1">
              <span>by {metadata.uploader_id}</span>
              <span>•</span>
              <span>{formatDate(metadata.created_at)}</span>
              <span>•</span>
              <span>#{metadata.id}</span>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* Copy to Editor button - only show when blocks are selected */}
          {hasSelection && onQuoteText && (
            <Button
              variant="default"
              size="sm"
              onClick={handleCopyToEditor}
              className="px-2 py-1 text-xs flex items-center gap-1"
            >
              <TextQuoteIcon className="h-3 w-3" />
              <span>
                Quote {contextSelectedBlockIndices.length} block
                {contextSelectedBlockIndices.length > 1 ? "s" : ""}
              </span>
            </Button>
          )}

          <Button
            variant="ghost"
            size="sm"
            onClick={() => setIsExpanded(!isExpanded)}
            className="shrink-0 px-2 py-1 text-xs text-blue-600 hover:text-white hover:bg-blue-600 flex items-center gap-1"
          >
            {isExpanded ? (
              <>
                <span>Collapse</span>
                <ChevronUpIcon className="h-3 w-3" />
              </>
            ) : (
              <>
                <span>Expand</span>
                <ChevronDownIcon className="h-3 w-3" />
              </>
            )}
          </Button>
        </div>
      </div>

      {/* Expandable Content */}
      {isExpanded && (
        <div className="overflow-auto max-h-[calc(40vh-4rem)]">
          {/* Document Content with Block Selection */}
          <div className="border-t">
            <DocumentContentWithSelection
              document={currentDocument}
              mode="context"
              showQuoteRail={true}
              contentClassName="p-4 prose prose-sm prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-lg prose-h2:text-base prose-h3:text-sm"
            />
          </div>
        </div>
      )}
    </div>
  );
}
