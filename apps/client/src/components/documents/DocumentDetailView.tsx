import { DocumentVerificationResult } from "@/lib/documentApi";
import { Route } from "@/routes/documents/document/$documentId";
import { Await } from "@tanstack/react-router";
import { useRef, useState } from "react";
import { useDocumentActions } from "../../hooks/useDocumentActions";
import { useDocumentSidebarState } from "../../hooks/useDocumentSidebarState";
import { useFileDownload } from "../../hooks/useFileDownload";
import { formatBlockQuotes, groupAdjacentBlocks } from "../../lib/blockUtils";
import { useDocuments } from "../../lib/store";
import { useSidebar } from "../ui/sidebar";
import { DocumentContent } from "./DocumentContent";
import { DocumentHeader } from "./DocumentHeader";
import { DocumentMetadata } from "./DocumentMetadata";
import { RepliesSection } from "./RepliesSection";
import { TableOfContents } from "./TableOfContents";
import { VerificationDisplay } from "./VerificationDisplay";

export function DocumentDetailView() {
  const { documentId } = Route.useParams();
  const loaderData = Route.useLoaderData();

  // Get block selection from store
  const { selectedBlockIndices, selectedBlockTexts } = useDocuments();

  const contentRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const { state: appSidebarState } = useSidebar();

  const [verificationResult, setVerificationResult] =
    useState<DocumentVerificationResult | null>(null);
  const [upvoteCount, setUpvoteCount] = useState<number>(
    loaderData.document.metadata.upvote_count
  );

  const {
    isVerifying,
    verificationError,
    isUpvoting,
    isDeleting,
    handleVerifyDocument,
    handleUpvote,
    handleDeleteDocument,
    handleReplyToDocument: handleReplyToDocumentBase,
    handleEditDocument,
    handleQuoteAndReply
  } = useDocumentActions(
    loaderData.document,
    setVerificationResult,
    setUpvoteCount
  );

  const { downloadingFiles, handleDownloadFile } = useFileDownload();

  // Enhanced reply handler that includes selected quotes
  const handleReplyToDocument = () => {
    // Get selected quote from store if any blocks are selected
    let selectedQuote: string | undefined;

    if (selectedBlockIndices.length > 0 && selectedBlockTexts.length > 0) {
      // Group adjacent blocks for better quote formatting
      const groups = groupAdjacentBlocks(selectedBlockIndices);
      const groupedBlockTexts = groups.map((group) =>
        group.map((index) => {
          const textIndex = selectedBlockIndices.indexOf(index);
          return selectedBlockTexts[textIndex] || "";
        })
      );

      // Format as quotes
      if (groups.length === 1) {
        // Single group of blocks
        selectedQuote = formatBlockQuotes(groupedBlockTexts[0]);
      } else {
        // Multiple groups - format each group and join with just the separator
        // formatBlockQuotes already adds "\n\n" at the end, so we just need to trim and join
        selectedQuote =
          groupedBlockTexts
            .map((groupBlocks) => formatBlockQuotes(groupBlocks).trimEnd())
            .join("\n\n") + "\n\n";
      }
    }

    handleReplyToDocumentBase(selectedQuote);
  };

  const { leftSidebarCollapsed, rightSidebarCollapsed } =
    useDocumentSidebarState();

  // Text selection replaced with block selection in DocumentContent component

  const isVerified = Boolean(
    verificationResult &&
      verificationResult.publish_verified &&
      verificationResult.timestamp_verified &&
      verificationResult.upvote_count_verified
  );

  return (
    <div className="flex min-h-screen w-full">
      {/* Left Sidebar - Table of Contents (Fixed) - Only show for message documents */}
      {loaderData.document.content.message && (
        <div
          className={`hidden lg:flex flex-col border-r bg-background fixed h-[calc(100vh-var(--top-bar-height))] z-10 ${
            leftSidebarCollapsed ? "w-0 overflow-hidden" : "w-64"
          }`}
          style={{
            top: "var(--top-bar-height)",
            left:
              appSidebarState === "expanded"
                ? "var(--sidebar-width)"
                : "var(--sidebar-width-icon)"
          }}
        >
          {!leftSidebarCollapsed && (
            <TableOfContents
              containerRef={contentRef}
              scrollContainerRef={scrollContainerRef}
              className="flex-1 overflow-y-auto"
            />
          )}
        </div>
      )}

      {/* Main Content Area */}
      <div
        ref={scrollContainerRef}
        className={`flex-1 min-w-0 p-6 ${
          leftSidebarCollapsed || !loaderData.document.content.message
            ? "lg:ml-0"
            : "lg:ml-64"
        } ${rightSidebarCollapsed ? "lg:mr-0" : "lg:mr-64"}`}
      >
        <div className="w-full max-w-4xl mx-auto">
          {/* Document Header */}
          <DocumentHeader
            currentDocument={loaderData.document}
            upvoteCount={upvoteCount}
            isUpvoting={isUpvoting}
            isVerifying={isVerifying}
            isDeleting={isDeleting}
            isVerified={isVerified}
            verificationResult={verificationResult}
            onUpvote={handleUpvote}
            onReply={handleReplyToDocument}
            onVerify={handleVerifyDocument}
            onEdit={handleEditDocument}
            onDelete={handleDeleteDocument}
          />

          {/* Show verification error if it exists */}
          <VerificationDisplay verificationError={verificationError} />

          {/* Document Content - Main Focus */}
          <div ref={contentRef}>
            <DocumentContent
              document={loaderData.document}
              downloadingFiles={downloadingFiles}
              onDownloadFile={handleDownloadFile}
              onQuoteText={handleQuoteAndReply}
            />
          </div>

          <Await promise={loaderData.replyTree}>
            {(replyTree) => (
              <RepliesSection
                replyTree={replyTree}
                repliesLoading={false}
                repliesError={null}
                documentId={Number(documentId)}
                postId={loaderData.document.metadata.post_id}
                rootPostTitle={loaderData.document.metadata.title}
              />
            )}
          </Await>
          {/* Replies Section */}
          {/* <RepliesSection
            replyTree={replyTree}
            repliesLoading={repliesLoading}
            repliesError={repliesError}
            documentId={documentId}
            postId={currentDocument.metadata.post_id}
            rootPostTitle={currentDocument.metadata.title}
          /> */}

          {/* Technical Details - Moved to Bottom */}
          <DocumentMetadata document={loaderData.document} />
        </div>
      </div>

      {/* Right Sidebar - Future Features (Fixed) */}
      <div
        className={`hidden lg:flex flex-col border-l bg-muted/30 fixed right-0 h-[calc(100vh-var(--top-bar-height))] z-10 ${
          rightSidebarCollapsed ? "w-0 overflow-hidden" : "w-64"
        }`}
        style={{
          top: "var(--top-bar-height)"
        }}
      >
        {!rightSidebarCollapsed && (
          <div className="p-4">
            <p className="text-sm text-muted-foreground text-center">
              Reserved for future features
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
