import {
  ChevronDownIcon,
  ChevronUpIcon,
  MessageSquareIcon
} from "lucide-react";
import { useEffect, useState } from "react";
import type { Document } from "../../lib/documentApi";
import { Button } from "../ui/button";
import { renderMarkdownToHtml, useMarkdownRenderer } from "./markdownRenderer";

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
  const md = useMarkdownRenderer();

  const { metadata, content } = currentDocument;

  // Get full content for expanded view
  const getFullContent = () => {
    if (content.message) {
      // Check if content looks like markdown
      const isMarkdown =
        content.message.includes("#") ||
        content.message.includes("**") ||
        content.message.includes("*") ||
        content.message.includes("```") ||
        (content.message.includes("[") && content.message.includes("](")) ||
        content.message.includes("- ") ||
        content.message.includes("1. ");

      if (isMarkdown) {
        return (
          <div
            className="prose prose-sm prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-lg prose-h2:text-base prose-h3:text-sm prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere"
            dangerouslySetInnerHTML={{
              __html: renderMarkdownToHtml(md, content.message)
            }}
          />
        );
      } else {
        return (
          <div className="prose prose-sm prose-neutral max-w-none dark:prose-invert">
            <p className="whitespace-pre-wrap text-sm">{content.message}</p>
          </div>
        );
      }
    } else if (content.file) {
      return (
        <div className="text-sm text-muted-foreground bg-muted/30 rounded p-3">
          <div className="font-medium">File: {content.file.name}</div>
          <div className="text-xs mt-1">
            {content.file.mime_type} •{" "}
            {(content.file.content.length / 1024).toFixed(1)} KB
          </div>
        </div>
      );
    } else if (content.url) {
      return (
        <div className="text-sm text-muted-foreground bg-muted/30 rounded p-3">
          <div className="font-medium">Link</div>
          <div className="text-xs mt-1 break-all">{content.url}</div>
        </div>
      );
    }

    return (
      <div className="text-sm text-muted-foreground italic">
        No content preview available
      </div>
    );
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

  // Text selection and quoting functionality
  useEffect(() => {
    if (!onQuoteText || !isExpanded) return;

    let quoteButtonElement: HTMLElement | null = null;

    const showQuoteButton = (selection: Selection, range: Range) => {
      // Remove any existing quote button
      if (quoteButtonElement) {
        document.body.removeChild(quoteButtonElement);
        quoteButtonElement = null;
      }

      const rect = range.getBoundingClientRect();
      const button = document.createElement("button");
      button.innerHTML = `
        <svg class="h-3 w-3 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24" style="display: inline; width: 12px; height: 12px; margin-right: 4px;">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/>
        </svg>
        Quote Text
      `;

      button.className =
        "fixed z-50 px-2 py-1 text-xs bg-blue-600 hover:bg-blue-700 text-white shadow-lg border border-blue-500 rounded select-none";
      button.style.left = `${rect.left + rect.width / 2 - 45}px`;
      button.style.top = `${rect.top - 35}px`;
      button.style.pointerEvents = "auto";

      button.onclick = () => {
        const selectedText = selection.toString().trim();
        if (selectedText && onQuoteText) {
          // Format as markdown quote
          const quotedText = selectedText
            .split("\n")
            .map((line) => `> ${line}`)
            .join("\n");

          onQuoteText(`${quotedText}\n\n`);
        }

        // Clean up
        if (quoteButtonElement) {
          document.body.removeChild(quoteButtonElement);
          quoteButtonElement = null;
        }
        selection.removeAllRanges();
      };

      document.body.appendChild(button);
      quoteButtonElement = button;
    };

    const hideQuoteButton = () => {
      if (quoteButtonElement) {
        document.body.removeChild(quoteButtonElement);
        quoteButtonElement = null;
      }
    };

    const handleMouseUp = () => {
      setTimeout(() => {
        const selection = window.getSelection();
        if (
          selection &&
          selection.rangeCount > 0 &&
          selection.toString().trim().length >= 3
        ) {
          const range = selection.getRangeAt(0);
          showQuoteButton(selection, range);
        } else {
          hideQuoteButton();
        }
      }, 100);
    };

    const handleClick = (e: MouseEvent) => {
      // Don't hide if clicking on our button
      if (
        (e.target as Element)
          ?.closest("button")
          ?.innerHTML?.includes("Quote Text")
      ) {
        return;
      }

      setTimeout(() => {
        const selection = window.getSelection();
        if (!selection || selection.toString().trim().length === 0) {
          hideQuoteButton();
        }
      }, 50);
    };

    document.addEventListener("mouseup", handleMouseUp);
    document.addEventListener("click", handleClick);

    return () => {
      document.removeEventListener("mouseup", handleMouseUp);
      document.removeEventListener("click", handleClick);
      hideQuoteButton();
    };
  }, [onQuoteText, isExpanded]);

  return (
    <div
      className={`border-l-4 border-l-blue-200 bg-blue-50/30 dark:bg-blue-950/10 dark:border-l-blue-800 ${className}`}
    >
      {/* Fixed Header Bar - Always Visible */}
      <div className="flex items-center justify-between px-4 py-3 border-b bg-blue-50/50 dark:bg-blue-950/20">
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <MessageSquareIcon className="h-4 w-4 text-blue-600 shrink-0" />
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-blue-900 dark:text-blue-100 line-clamp-1">
              Replying to: {metadata.title || "Untitled"}
            </div>
            <div className="flex items-center gap-2 text-xs text-blue-700 dark:text-blue-300 mt-1">
              <span>by {metadata.uploader_id}</span>
              <span>•</span>
              <span>{formatDate(metadata.created_at)}</span>
              <span>•</span>
              <span>#{metadata.id}</span>
            </div>
          </div>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setIsExpanded(!isExpanded)}
          className="shrink-0 px-2 py-1 text-xs text-blue-600 hover:text-white hover:bg-blue-600 ml-2 flex items-center gap-1"
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

      {/* Expandable Content */}
      {isExpanded && (
        <div className="p-4 space-y-3 overflow-auto max-h-[calc(45vh-4rem)]">
          {/* Tags */}
          {metadata.tags.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {metadata.tags.map((tag, index) => (
                <span
                  key={index}
                  className="inline-block px-2 py-1 text-xs bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-200 rounded"
                >
                  {tag}
                </span>
              ))}
            </div>
          )}

          {/* Authors */}
          {metadata.authors.length > 0 && (
            <div className="text-xs text-muted-foreground">
              <span>Authors: </span>
              {metadata.authors.join(", ")}
            </div>
          )}

          {/* Content */}
          <div className="border-t pt-3">{getFullContent()}</div>
        </div>
      )}
    </div>
  );
}
