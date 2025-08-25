import { useEffect, useMemo, useRef } from "react";
import { isMarkdownContent } from "../../lib/contentUtils";
import { Document } from "../../lib/documentApi";
import { useDocuments } from "../../lib/store";
import {
  renderMarkdownWithBlocks,
  useMarkdownRenderer
} from "./markdownRenderer";
import { QuoteRail } from "./QuoteRail";

export type SelectionMode = "view" | "context";

interface DocumentContentWithSelectionProps {
  document: Document;
  mode?: SelectionMode;
  showQuoteRail?: boolean;
  className?: string;
  contentClassName?: string;
}

export function DocumentContentWithSelection({
  document,
  mode = "view",
  showQuoteRail = true,
  className = "",
  contentClassName = ""
}: DocumentContentWithSelectionProps) {
  const md = useMarkdownRenderer();
  const contentRef = useRef<HTMLDivElement>(null);
  const renderedContentRef = useRef<HTMLDivElement>(null);

  // Get the appropriate selection state from store based on mode
  const {
    selectedBlockIndices,
    setSelectedBlockTexts,
    setViewingDocument,
    contextSelectedBlockIndices,
    setContextSelectedBlockTexts,
    setContextDocument
  } = useDocuments();

  // Use different state based on mode
  const activeSelectedIndices =
    mode === "view" ? selectedBlockIndices : contextSelectedBlockIndices;
  const setActiveSelectedTexts =
    mode === "view" ? setSelectedBlockTexts : setContextSelectedBlockTexts;
  const setActiveDocument =
    mode === "view" ? setViewingDocument : setContextDocument;

  // Memoize rendered HTML and blocks for message content
  const renderedMessageData = useMemo(() => {
    if (!document.content.message) return null;

    const isMarkdown = isMarkdownContent(document.content.message);

    if (isMarkdown) {
      const { html, blocks } = renderMarkdownWithBlocks(
        md,
        document.content.message
      );
      return { html, blocks, isMarkdown };
    }

    return { html: null, blocks: [], isMarkdown };
  }, [document.content.message, md]);

  // Set viewing/context document on mount
  useEffect(() => {
    if (document.metadata.id) {
      setActiveDocument(document.metadata.id);
    }
    return () => {
      setActiveDocument(null);
    };
  }, [document.metadata.id, setActiveDocument]);

  // Update selected block texts when selection changes
  useEffect(() => {
    if (activeSelectedIndices.length > 0 && renderedMessageData?.blocks) {
      const texts = activeSelectedIndices.map(
        (idx) => renderedMessageData.blocks[idx] || ""
      );
      setActiveSelectedTexts(texts);
    } else {
      setActiveSelectedTexts([]);
    }
  }, [
    activeSelectedIndices,
    renderedMessageData?.blocks,
    setActiveSelectedTexts
  ]);

  // Memoize the rendered content to prevent re-rendering on selection changes
  const renderedContent = useMemo(() => {
    if (!document.content.message) return null;

    if (renderedMessageData?.isMarkdown) {
      return (
        <div
          ref={renderedContentRef}
          className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere"
          dangerouslySetInnerHTML={{ __html: renderedMessageData?.html || "" }}
        />
      );
    } else {
      return (
        <div
          ref={renderedContentRef}
          className="prose prose-neutral max-w-none dark:prose-invert"
        >
          <p className="whitespace-pre-wrap">{document.content.message}</p>
        </div>
      );
    }
  }, [document.content.message, renderedMessageData]);

  // Add highlighting effect to selected blocks
  useEffect(() => {
    // Use either the rendered content ref or the content ref
    const searchContainer = renderedContentRef.current || contentRef.current;
    if (!searchContainer) return;

    // Remove any existing highlights
    const existingHighlights =
      searchContainer.querySelectorAll(".block-selected");
    existingHighlights.forEach((el) => {
      el.classList.remove("block-selected");
    });

    // Add highlights to selected blocks
    activeSelectedIndices.forEach((blockIndex) => {
      const blockElement = searchContainer.querySelector(
        `[data-block-index="${blockIndex}"]`
      );
      if (blockElement) {
        blockElement.classList.add("block-selected");
      }
    });
  }, [activeSelectedIndices]);

  return (
    <div className={`relative p-1 ${className}`}>
      {/* Add CSS for block highlighting */}
      <style>{`
        .block-selected {
          background-color: rgba(59, 130, 246, 0.1) !important;
          border-left: 3px solid rgb(59, 130, 246) !important;
          padding-left: calc(1.5rem - 3px) !important;
          transition: all 0.2s ease !important;
        }
        .dark .block-selected {
          background-color: rgba(59, 130, 246, 0.15) !important;
        }
      `}</style>

      {/* Render message content if it exists */}
      {document.content.message && (
        <div
          className={`relative ${
            renderedMessageData?.isMarkdown &&
            renderedMessageData.blocks.length > 0 &&
            showQuoteRail
              ? "ml-12"
              : ""
          }`}
        >
          {/* Quote Rail - Only show for markdown content with blocks */}
          {renderedMessageData?.isMarkdown &&
            renderedMessageData.blocks.length > 0 &&
            showQuoteRail && (
              <QuoteRail
                contentRef={contentRef}
                className="left-[-48px]"
                mode={mode}
              />
            )}

          <div
            ref={contentRef}
            className={
              contentClassName ||
              "bg-white dark:bg-gray-900 rounded-lg border p-6"
            }
          >
            {renderedContent}
          </div>
        </div>
      )}
    </div>
  );
}
