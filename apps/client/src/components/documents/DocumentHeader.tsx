import { CheckCircleIcon, EditIcon, ReplyIcon, TrashIcon } from "lucide-react";
import { formatDate } from "../../lib/dateUtils";
import { Document } from "../../lib/documentApi";
import { TopBarSlot } from "../TopBarContext";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { useDocuments } from "../../lib/store";

interface DocumentHeaderProps {
  currentDocument: Document;
  upvoteCount: number;
  currentUsername: string | null;
  isUpvoting: boolean;
  isVerifying: boolean;
  isDeleting: boolean;
  isVerified: boolean;
  verificationResult: any;
  onUpvote: () => void;
  onReply: () => void;
  onVerify: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

export function DocumentHeader({
  currentDocument,
  upvoteCount,
  currentUsername,
  isUpvoting,
  isVerifying,
  isDeleting,
  isVerified,
  verificationResult,
  onUpvote,
  onReply,
  onVerify,
  onEdit,
  onDelete
}: DocumentHeaderProps) {
  const isOwner =
    currentUsername && currentDocument.metadata.uploader_id === currentUsername;

  // Get block selection state from store
  const { selectedBlockIndices } = useDocuments();
  const quotesSelected = selectedBlockIndices.length > 0;

  return (
    <div className="mb-6">
      <div className="flex items-start gap-4">
        {/* Upvote section */}
        <div className="flex flex-col items-center min-w-[80px] pt-2">
          <button
            onClick={onUpvote}
            disabled={isUpvoting}
            className={`text-3xl mb-2 transition-colors ${
              isUpvoting
                ? "text-muted-foreground cursor-not-allowed"
                : "text-muted-foreground hover:text-orange-500 cursor-pointer"
            }`}
            title="Upvote this document"
          >
            ▲
          </button>
          <div className="text-2xl font-bold text-orange-500 mb-1">
            {upvoteCount}
          </div>
          <div className="text-xs text-muted-foreground">upvotes</div>
        </div>

        {/* Main content header */}
        <div className="flex-1 min-w-0">
          {/* Title */}
          <h1 className="text-3xl font-bold text-foreground mb-4 line-clamp-3">
            {currentDocument.metadata.title}
          </h1>

          {/* Author/Uploader info */}
          <div className="flex items-center gap-2 text-sm text-muted-foreground mb-4">
            <span>Posted by</span>
            <span className="font-medium text-blue-600">
              {currentDocument.metadata.uploader_id}
            </span>
            <span>•</span>
            <span>{formatDate(currentDocument.metadata.created_at)}</span>
            {currentDocument.metadata.reply_to && (
              <>
                <span>•</span>
                <span className="text-orange-600">
                  Reply to #{currentDocument.metadata.reply_to.document_id}{" "}
                  (Post {currentDocument.metadata.reply_to.post_id})
                </span>
              </>
            )}
          </div>

          {/* Authors (if different from uploader) */}
          {currentDocument.metadata.authors.length > 0 && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground mb-4">
              <span>Authors:</span>
              <div className="flex gap-2">
                {currentDocument.metadata.authors.map((author, index) => (
                  <Badge
                    key={index}
                    variant="secondary"
                    className="text-xs bg-blue-100 text-blue-800"
                  >
                    {author}
                  </Badge>
                ))}
              </div>
            </div>
          )}

          {/* Tags */}
          {currentDocument.metadata.tags.length > 0 && (
            <div className="flex items-center gap-2 mb-6">
              {currentDocument.metadata.tags.map((tag, index) => (
                <Badge
                  key={index}
                  variant="outline"
                  className="text-xs bg-green-50 text-green-700 border-green-200 hover:bg-green-100"
                >
                  {tag}
                </Badge>
              ))}
            </div>
          )}
        </div>

        {/* Right side - Action buttons */}
        <div className="flex flex-col items-end gap-2">
          {/* Sidebar Toggle Buttons */}
          {/* <div className="hidden lg:flex items-center gap-1">
            {currentDocument.content.message && (
              <Button
                onClick={onToggleLeftSidebar}
                variant="ghost"
                size="sm"
                title="Toggle Table of Contents"
              >
                <PanelLeftIcon className="h-3 w-3" />
              </Button>
            )}
            <Button
              onClick={onToggleRightSidebar}
              variant="ghost"
              size="sm"
              title="Toggle Right Panel"
            >
              <PanelRightIcon className="h-3 w-3" />
            </Button>
          </div> */}
          <TopBarSlot position="right">
            <div className="flex items-center gap-2">
              <Button
                onClick={onReply}
                variant={quotesSelected ? "default" : "outline"}
                size="sm"
              >
                <ReplyIcon className="h-3 w-3 mr-1" />
                {quotesSelected ? "Quote" : "Reply"}
              </Button>
              <Button
                onClick={onVerify}
                disabled={isVerifying}
                variant="outline"
                size="sm"
              >
                <>
                  <CheckCircleIcon
                    className={`h-3 w-3 mr-1 ${isVerifying ? "animate-spin" : isVerified ? "text-green-600" : verificationResult ? "text-red-600" : ""}`}
                  />
                  {isVerified
                    ? "Verified"
                    : verificationResult
                      ? "Verification Failed"
                      : "Verify"}
                </>
              </Button>
              {/* Edit button - only show for document owner */}
              {isOwner && (
                <Button
                  onClick={onEdit}
                  variant="outline"
                  size="sm"
                  className="text-blue-600 hover:text-blue-700 hover:bg-blue-50 border-blue-200"
                >
                  <EditIcon className="h-3 w-3 mr-1" />
                  Edit
                </Button>
              )}
              {/* Delete button - only show for document owner */}
              {isOwner && (
                <Button
                  onClick={onDelete}
                  disabled={isDeleting}
                  variant="outline"
                  size="sm"
                  className="text-red-600 hover:text-red-700 hover:bg-red-50 border-red-200"
                >
                  {isDeleting ? (
                    <>
                      <div className="animate-spin rounded-full h-3 w-3 border-b border-current mr-1"></div>
                      Deleting...
                    </>
                  ) : (
                    <>
                      <TrashIcon className="h-3 w-3 mr-1" />
                      Delete
                    </>
                  )}
                </Button>
              )}
            </div>
          </TopBarSlot>
        </div>
      </div>
    </div>
  );
}
