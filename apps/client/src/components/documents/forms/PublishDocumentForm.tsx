import { Trash2Icon } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { fetchDocument, type Document } from "../../../lib/documentApi";
import { useDraftAutoSave, type DraftContent } from "../../../lib/drafts";
import { Button } from "../../ui/button";
import { InlineChipInput } from "../../ui/inline-chip-input";
import { ContextPreview } from "../ContextPreview";
import { PublishButton } from "../PublishButton";
import { MarkdownEditor } from "../editors/MarkdownEditor";
import type { EditingDocument } from "../PublishPage";

interface PublishDocumentFormProps {
  onPublishSuccess?: (documentId: number) => void;
  onCancel?: () => void;
  replyTo?: string;
  editingDraftId?: string; // UUID
  editingDocument?: EditingDocument; // For editing existing documents
}

export function PublishDocumentForm({
  onPublishSuccess,
  onCancel,
  replyTo,
  editingDraftId,
  editingDocument
}: PublishDocumentFormProps) {
  const navigate = useNavigate();
  const navigateToDrafts = () => navigate({ to: "/documents/drafts" });
  const navigateToDocument = (documentId: number) => {
    navigate({
      to: "/documents/document/$documentId",
      params: { documentId: documentId.toString() }
    });
  };
  const goBack = () => navigate({ to: ".." });

  // Check if we're editing an existing document
  const isEditingDocument = !!editingDocument;

  // Use the draft auto-save hook
  const {
    currentDraftId,
    initialContent,
    markContentChanged,
    discardDraft,
    markSaved,
    registerContentGetter
  } = useDraftAutoSave({ editingDraftId });

  // Form state - initialize from editing document, draft, or defaults
  const [title, setTitle] = useState(
    editingDocument?.title ?? initialContent?.title ?? ""
  );
  const [titleTouched, setTitleTouched] = useState(false);
  const [message, setMessage] = useState(
    editingDocument?.content.message ?? initialContent?.message ?? ""
  );
  const [tags, setTags] = useState<string[]>(
    editingDocument?.tags ?? initialContent?.tags ?? []
  );
  const [authors, setAuthors] = useState<string[]>(
    editingDocument?.authors ?? initialContent?.authors ?? []
  );
  const [replyToDocument, setReplyToDocument] = useState<Document | null>(null);
  const [replyToLoading, setReplyToLoading] = useState(false);

  // Update form state when initial content loads (but not when editing document)
  useEffect(() => {
    if (initialContent && !isEditingDocument) {
      setTitle(initialContent.title);
      setMessage(initialContent.message);
      setTags(initialContent.tags);
      setAuthors(initialContent.authors);
    }
  }, [initialContent, isEditingDocument]);

  // Helper functions that mark content as changed
  const handleTagsChange = (newTags: string[]) => {
    setTags(newTags);
    markContentChanged();
  };

  const handleAuthorsChange = (newAuthors: string[]) => {
    setAuthors(newAuthors);
    markContentChanged();
  };

  // Get current form content as DraftContent
  const getCurrentContent = (): DraftContent => ({
    title,
    message,
    tags,
    authors,
    replyTo
  });

  const getPublishData = () => {
    const data: any = {
      title: title.trim(),
      message: message.trim(),
      tags: tags.length > 0 ? tags : undefined,
      authors: authors.length > 0 ? authors : undefined,
      replyTo: editingDocument?.replyTo ?? replyTo,
      draftId: currentDraftId || editingDraftId, // Pass the draft ID for deletion after publish
      postId: editingDocument?.postId // Include postId when editing existing document
    };

    return data;
  };

  const handleSubmitAttempt = () => {
    setTitleTouched(true);
  };

  const isValid = () => {
    // Title and message are mandatory for documents
    return title.trim().length > 0 && message.trim().length > 0;
  };

  const handleDiscardDraft = async () => {
    const success = await discardDraft();
    if (success) {
      // Navigate based on context priority:

      // 1. Modal context - highest priority (user expects modal behavior)
      if (onCancel) {
        onCancel();

        // 2. Reply context - navigate back to the document being replied to
        // Check editing document, props replyTo, and draft's replyTo
      } else if (effectiveReplyTo || initialContent?.replyTo) {
        const replyToId = effectiveReplyTo || initialContent?.replyTo;
        if (replyToId) {
          try {
            // replyTo format is "post_id:document_id", we want the document_id
            const documentId = parseInt(replyToId.split(":")[1]);
            if (!isNaN(documentId)) {
              navigateToDocument(documentId);
            } else {
              // If we can't parse document ID, use history navigation
              goBack();
            }
          } catch (error) {
            console.error("Failed to parse replyTo document ID:", error);
            goBack();
          }
        } else {
          goBack();
        }

        // 3. Existing draft context - navigate back to drafts list
      } else if (editingDraftId) {
        navigateToDrafts();

        // 4. Fallback - use browser-like back navigation
      } else {
        goBack();
      }
    }
  };

  // Register content getter with the hook for save-on-unmount
  useEffect(() => {
    registerContentGetter(getCurrentContent);
  }, [registerContentGetter, title, message, tags, authors, replyTo]); // Re-register when content changes

  // Initialize reply context from editing document if present
  const effectiveReplyTo = editingDocument?.replyTo ?? replyTo;

  useEffect(() => {
    if (effectiveReplyTo && !replyToDocument && !replyToLoading) {
      setReplyToLoading(true);
      // Extract document_id from "post_id:document_id" format
      const documentId = parseInt(effectiveReplyTo.split(":")[1]);
      fetchDocument(documentId)
        .then((doc) => {
          setReplyToDocument(doc);
        })
        .catch((error) => {
          console.error("Failed to fetch reply-to document:", error);
        })
        .finally(() => {
          setReplyToLoading(false);
        });
    }
  }, [effectiveReplyTo, replyToDocument, replyToLoading]);

  // This effect is now handled above in the effectiveReplyTo useEffect

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Top Bar */}
      <div className="flex items-center gap-6 px-6 py-4 border-b bg-background shrink-0">
        {/* Title - Direct Input */}
        <div className="flex-1 min-w-0">
          <input
            type="text"
            placeholder="Untitled Document"
            value={title}
            onChange={(e) => {
              setTitle(e.target.value);
              markContentChanged();
            }}
            onBlur={() => setTitleTouched(true)}
            autoComplete="off"
            autoCorrect="off"
            maxLength={200}
            className={`w-full bg-transparent border-none outline-none text-xl font-semibold placeholder:text-muted-foreground ${
              titleTouched && title.trim().length === 0
                ? "text-destructive"
                : "text-foreground"
            }`}
          />
          {titleTouched && title.trim().length === 0 && (
            <p className="text-sm text-destructive mt-1">Title is required</p>
          )}
        </div>

        {/* Tags Section */}
        <InlineChipInput
          label="Tags"
          placeholder="Add tags..."
          values={tags}
          onValuesChange={handleTagsChange}
        />

        {/* Authors Section */}
        <InlineChipInput
          label="Authors"
          placeholder="Add authors..."
          values={authors}
          onValuesChange={handleAuthorsChange}
        />

        {/* Action Buttons */}
        <div className="flex items-center gap-3 shrink-0">
          <Button
            variant="outline"
            onClick={handleDiscardDraft}
            className="flex items-center gap-2 text-destructive hover:text-destructive"
          >
            <Trash2Icon className="h-4 w-4" />
            Discard
          </Button>

          <PublishButton
            data={getPublishData()}
            disabled={!isValid()}
            onPublishSuccess={(documentId) => {
              console.log("onPublishSuccess called");
              markSaved(); // Mark as saved using the hook
              // Call the original success callback
              if (onPublishSuccess) {
                onPublishSuccess(documentId);
              }
            }}
            onSubmitAttempt={handleSubmitAttempt}
          />
        </div>
      </div>

      {/* Reply Context Preview */}
      {effectiveReplyTo && replyToDocument && (
        <div className="shrink-0 border-b">
          <ContextPreview
            document={replyToDocument}
            onQuoteText={(quotedText) => {
              // Append quoted text to current message
              const currentMessage = message;
              const newMessage = currentMessage
                ? `${currentMessage}\n\n${quotedText}`
                : quotedText;
              setMessage(newMessage);
              markContentChanged();
            }}
          />
        </div>
      )}

      {effectiveReplyTo && replyToLoading && (
        <div className="px-6 py-2 bg-muted/50 border-b shrink-0">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <div className="animate-spin rounded-full h-4 w-4 border-b border-current"></div>
            <span>Loading document details...</span>
          </div>
        </div>
      )}

      {/* Markdown Editor - Full Height */}
      <div className="flex-1 min-h-0 overflow-hidden">
        <MarkdownEditor
          value={message}
          onChange={(value) => {
            setMessage(value);
            markContentChanged();
          }}
          className="h-full"
        />
      </div>
    </div>
  );
}
