import { useMemo, useRef, useEffect } from "react";
import { DownloadIcon, ExternalLinkIcon, FileTextIcon } from "lucide-react";
import { Document, DocumentFile } from "../../lib/documentApi";
import {
  isImageFile,
  isMarkdownFile,
  isTextFile,
  fileContentToString,
  fileContentToDataUrl
} from "../../lib/fileUtils";
import { isMarkdownContent } from "../../lib/contentUtils";
import {
  renderMarkdownToHtml,
  renderMarkdownWithBlocks,
  useMarkdownRenderer
} from "./markdownRenderer";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { QuoteRail } from "./QuoteRail";
import { useBlockSelection } from "../../hooks/useBlockSelection";

interface DocumentContentProps {
  document: Document;
  downloadingFiles: Set<string>;
  onDownloadFile: (file: DocumentFile) => Promise<void>;
  onQuoteText?: (text: string) => Promise<void>;
}

export function DocumentContent({
  document,
  downloadingFiles,
  onDownloadFile,
  onQuoteText
}: DocumentContentProps) {
  const md = useMarkdownRenderer();
  const contentRef = useRef<HTMLDivElement>(null);

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

  // Block selection for quoting
  const blockSelection = useBlockSelection({
    blocks: renderedMessageData?.blocks || [],
    onQuoteText
  });

  const renderContent = (content: Document["content"]) => {
    if (!content.message) return null;

    if (renderedMessageData?.isMarkdown) {
      return (
        <div
          className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere"
          dangerouslySetInnerHTML={{ __html: renderedMessageData.html! }}
        />
      );
    } else {
      return (
        <div className="prose prose-neutral max-w-none dark:prose-invert">
          <p className="whitespace-pre-wrap">{content.message}</p>
        </div>
      );
    }
  };

  const renderFileAttachment = (file: DocumentFile) => {
    if (!file) return null;

    const isImage = isImageFile(file.mime_type);
    const isMarkdown = isMarkdownFile(file.mime_type, file.name);
    const isText = isTextFile(file.mime_type);

    // For markdown files, render the content directly
    if (
      isMarkdown ||
      (isText &&
        (file.name.toLowerCase().endsWith(".md") ||
          file.name.toLowerCase().endsWith(".markdown")))
    ) {
      // Convert byte array to string properly
      const fileContent = fileContentToString(file.content);
      console.log("📁 File detected as markdown:", file.name);
      console.log("📁 File MIME type:", file.mime_type);
      console.log("📁 File content length:", file.content.length);
      console.log(
        "📁 File content preview:",
        fileContent.substring(0, 100) + "..."
      );
      console.log("📁 isMarkdown flag:", isMarkdown);
      console.log("📁 isText flag:", isText);

      return (
        <div className="mt-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-medium flex items-center gap-2">
              <FileTextIcon className="h-5 w-5" />
              {file.name}
            </h3>
            <div className="flex items-center gap-2">
              <Badge variant="outline" className="text-xs">
                {file.mime_type} • {(file.content.length / 1024).toFixed(1)} KB
              </Badge>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onDownloadFile(file)}
                disabled={downloadingFiles.has(
                  `${file.name}_${file.mime_type}`
                )}
              >
                <DownloadIcon className="h-4 w-4 mr-2" />
                {downloadingFiles.has(`${file.name}_${file.mime_type}`)
                  ? "Downloading..."
                  : "Download"}
              </Button>
            </div>
          </div>

          <div
            className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere border rounded-lg p-6 max-h-[70vh] overflow-y-auto"
            dangerouslySetInnerHTML={{
              __html: renderMarkdownToHtml(md, fileContent)
            }}
          />
        </div>
      );
    }

    // For other text files, show as plain text
    if (isText && !isMarkdown) {
      const fileContent = fileContentToString(file.content);

      return (
        <div className="mt-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-medium flex items-center gap-2">
              <FileTextIcon className="h-5 w-5" />
              {file.name}
            </h3>
            <div className="flex items-center gap-2">
              <Badge variant="outline" className="text-xs">
                {file.mime_type} • {(file.content.length / 1024).toFixed(1)} KB
              </Badge>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onDownloadFile(file)}
                disabled={downloadingFiles.has(
                  `${file.name}_${file.mime_type}`
                )}
              >
                <DownloadIcon className="h-4 w-4 mr-2" />
                {downloadingFiles.has(`${file.name}_${file.mime_type}`)
                  ? "Downloading..."
                  : "Download"}
              </Button>
            </div>
          </div>

          <div className="border rounded-lg p-6 bg-muted/20 max-h-[70vh] overflow-y-auto">
            <pre className="whitespace-pre-wrap font-mono text-sm overflow-x-auto">
              {fileContent}
            </pre>
          </div>
        </div>
      );
    }

    // For non-text files (images, binaries, etc.), show as attachment
    return (
      <Card className="mt-4">
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            <FileTextIcon className="h-5 w-5" />
            File Attachment
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div>
              <p className="font-medium">{file.name}</p>
              <p className="text-sm text-muted-foreground">
                {file.mime_type} • {(file.content.length / 1024).toFixed(1)} KB
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onDownloadFile(file)}
              disabled={downloadingFiles.has(`${file.name}_${file.mime_type}`)}
            >
              <DownloadIcon
                className={`h-4 w-4 mr-2 ${downloadingFiles.has(`${file.name}_${file.mime_type}`) ? "animate-spin" : ""}`}
              />
              {downloadingFiles.has(`${file.name}_${file.mime_type}`)
                ? "Downloading..."
                : "Download"}
            </Button>
          </div>

          {isImage && (
            <div className="mt-4">
              <img
                src={fileContentToDataUrl(file.content, file.mime_type)}
                alt={file.name}
                className="max-w-full h-auto rounded-lg border"
              />
            </div>
          )}
        </CardContent>
      </Card>
    );
  };

  const renderUrl = (url: string) => (
    <Card className="mt-4">
      <CardContent className="pt-6">
        <div className="flex items-center gap-2">
          <ExternalLinkIcon className="h-4 w-4" />
          <span className="font-medium">Referenced URL:</span>
          <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-blue-600 hover:text-blue-800 underline"
          >
            {url}
          </a>
        </div>
      </CardContent>
    </Card>
  );

  // Add highlighting effect to selected blocks
  useEffect(() => {
    if (!contentRef.current) return;

    // Remove any existing highlights
    const existingHighlights =
      contentRef.current.querySelectorAll(".block-selected");
    existingHighlights.forEach((el) => {
      el.classList.remove("block-selected");
    });

    // Add highlights to selected blocks
    blockSelection.selectedBlocks.forEach((blockIndex) => {
      const blockElement = contentRef.current?.querySelector(
        `[data-block-index="${blockIndex}"]`
      );
      if (blockElement) {
        blockElement.classList.add("block-selected");
      }
    });
  }, [blockSelection.selectedBlocks]);

  return (
    <div className="relative mb-8 select-text">
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
          className={`relative ${renderedMessageData?.isMarkdown && renderedMessageData.blocks.length > 0 && onQuoteText ? "ml-12" : ""}`}
        >
          {/* Quote Rail - Only show for markdown content with blocks */}
          {renderedMessageData?.isMarkdown &&
            renderedMessageData.blocks.length > 0 &&
            onQuoteText && (
              <QuoteRail
                contentRef={contentRef}
                selectedBlocks={blockSelection.selectedBlocks}
                onBlockToggle={blockSelection.toggleBlock}
                onQuoteSelected={blockSelection.handleQuoteSelected}
                className="left-[-48px]"
              />
            )}

          <div
            ref={contentRef}
            className="bg-white dark:bg-gray-900 rounded-lg border p-6 mb-6"
          >
            {renderContent(document.content)}
          </div>
        </div>
      )}

      {/* If no message content but there's a file, render the file content */}
      {!document.content.message &&
        document.content.file &&
        renderFileAttachment(document.content.file)}

      {/* If there's both message and file, render file as attachment */}
      {document.content.message &&
        document.content.file &&
        renderFileAttachment(document.content.file)}

      {/* Render URL if it exists */}
      {document.content.url && renderUrl(document.content.url)}

      {/* Show empty state only if no content at all */}
      {!document.content.message &&
        !document.content.file &&
        !document.content.url && (
          <div className="text-center py-8 text-muted-foreground bg-muted/30 rounded-lg">
            <FileTextIcon className="h-12 w-12 mx-auto mb-2" />
            <p>Content not available or unsupported format</p>
          </div>
        )}
    </div>
  );
}
