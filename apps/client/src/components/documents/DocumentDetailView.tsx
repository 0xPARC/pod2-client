import "highlight.js/styles/github-dark.css";
import { AlertCircleIcon } from "lucide-react";
import { useRef } from "react";
import { useDocumentActions } from "../../hooks/useDocumentActions";
import { useDocumentData } from "../../hooks/useDocumentData";
import { useDocumentSidebarState } from "../../hooks/useDocumentSidebarState";
import { useFileDownload } from "../../hooks/useFileDownload";
import { formatBlockQuotes, groupAdjacentBlocks } from "../../lib/blockUtils";
import { useDocuments } from "../../lib/store";
import { Card, CardContent } from "../ui/card";
import { useSidebar } from "../ui/sidebar";
import { DocumentContent } from "./DocumentContent";
import { DocumentHeader } from "./DocumentHeader";
import { DocumentMetadata } from "./DocumentMetadata";
import { RepliesSection } from "./RepliesSection";
import { TableOfContents } from "./TableOfContents";
import { VerificationDisplay } from "./VerificationDisplay";

interface DocumentDetailViewProps {
  documentId: number;
  onNavigateToDocument?: (documentId: number) => void;
}

export function DocumentDetailView({
  documentId,
  onNavigateToDocument
}: DocumentDetailViewProps) {
  const {
    navigateToPublish,
    navigateToDocumentsList,
    updateCurrentRouteTitle
  } = useDocuments();

  const contentRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const { state: appSidebarState } = useSidebar();

  // Get block selection from store
  const { selectedBlockIndices, selectedBlockTexts } = useDocuments();

  // Custom hooks for data and state management
  const {
    currentDocument,
    loading,
    error,
    verificationResult,
    replies,
    repliesLoading,
    repliesError,
    currentUsername,
    upvoteCount,
    setUpvoteCount,
    setVerificationResult
  } = useDocumentData(documentId, updateCurrentRouteTitle);

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
    currentDocument,
    setVerificationResult,
    setUpvoteCount,
    navigateToPublish,
    navigateToDocumentsList
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

  if (loading) {
    return (
      <div className="p-6 min-h-screen w-full">
        <div className="w-full">
          <Card>
            <CardContent className="pt-6">
              <div className="flex items-center justify-center py-12">
                <div className="text-center">
                  <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-4"></div>
                  Loading document...
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }

  if (error || !currentDocument) {
    return (
      <div className="p-6 min-h-screen w-full">
        <div className="w-full">
          <Card className="border-destructive">
            <CardContent className="pt-6">
              <div className="flex items-center gap-2 text-destructive">
                <AlertCircleIcon className="h-5 w-5" />
                <span>{error || "Document not found"}</span>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen w-full">
      {/* Left Sidebar - Table of Contents (Fixed) - Only show for message documents */}
      {currentDocument.content.message && (
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
          leftSidebarCollapsed || !currentDocument.content.message
            ? "lg:ml-0"
            : "lg:ml-64"
        } ${rightSidebarCollapsed ? "lg:mr-0" : "lg:mr-64"}`}
      >
        <div className="w-full max-w-4xl mx-auto">
          {/* Document Header */}
          <DocumentHeader
            currentDocument={currentDocument}
            upvoteCount={upvoteCount}
            currentUsername={currentUsername}
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
              document={currentDocument}
              downloadingFiles={downloadingFiles}
              onDownloadFile={handleDownloadFile}
              onQuoteText={handleQuoteAndReply}
            />
          </div>

          {/* Replies Section */}
          <RepliesSection
            replies={replies}
            repliesLoading={repliesLoading}
            repliesError={repliesError}
            documentId={documentId}
            postId={currentDocument.metadata.post_id}
            onNavigateToDocument={onNavigateToDocument}
          />

          {/* Technical Details - Moved to Bottom */}
          <DocumentMetadata document={currentDocument} />
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
