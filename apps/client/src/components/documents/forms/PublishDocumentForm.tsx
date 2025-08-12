import { MessageSquareIcon, Trash2Icon } from "lucide-react";
import { useEffect, useState } from "react";
import { fetchDocument, type DocumentMetadata } from "../../../lib/documentApi";
import { useDraftAutoSave, type DraftContent } from "../../../lib/drafts";
import { useDocuments } from "../../../lib/store";
import { Button } from "../../ui/button";
import { InlineChipInput } from "../../ui/inline-chip-input";
import { PublishButton } from "../PublishButton";
import { MarkdownEditor } from "../editors/MarkdownEditor";

interface PublishDocumentFormProps {
  onPublishSuccess?: (documentId: number) => void;
  onCancel?: () => void;
  replyTo?: string;
  editingDraftId?: string; // UUID
}

export function PublishDocumentForm({
  onPublishSuccess,
  onCancel,
  replyTo,
  editingDraftId
}: PublishDocumentFormProps) {
  const { currentRoute, navigateToDrafts } = useDocuments();

  // Get route-specific edit document data
  const editDocumentData = currentRoute?.editDocumentData;

  // Use the draft auto-save hook
  const {
    currentDraftId,
    initialContent,
    markContentChanged,
    discardDraft,
    markSaved,
    registerContentGetter
  } = useDraftAutoSave({ editingDraftId });

  // Form state - initialize from loaded draft or defaults
  const [title, setTitle] = useState(initialContent?.title ?? "");
  const [titleTouched, setTitleTouched] = useState(false);
  const [message, setMessage] = useState(initialContent?.message ?? "");
  const [tags, setTags] = useState<string[]>(initialContent?.tags ?? []);
  const [authors, setAuthors] = useState<string[]>(
    initialContent?.authors ?? []
  );
  const [replyToDocument, setReplyToDocument] =
    useState<DocumentMetadata | null>(null);
  const [replyToLoading, setReplyToLoading] = useState(false);

  // Update form state when initial content loads
  useEffect(() => {
    if (initialContent) {
      setTitle(initialContent.title);
      setMessage(initialContent.message);
      setTags(initialContent.tags);
      setAuthors(initialContent.authors);
    }
  }, [initialContent]);

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
      replyTo,
      draftId: currentDraftId || editingDraftId, // Pass the draft ID for deletion after publish
      postId: editDocumentData?.postId // Pass post ID for editing documents (creating revisions)
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
      // Navigate based on context
      if (editingDraftId) {
        navigateToDrafts();
      } else if (onCancel) {
        onCancel();
      }
    }
  };

  // Register content getter with the hook for save-on-unmount
  useEffect(() => {
    registerContentGetter(getCurrentContent);
  }, [registerContentGetter, title, message, tags, authors, replyTo]); // Re-register when content changes

  // Load document data if editing a document
  useEffect(() => {
    const loadEditDocument = async () => {
      if (editDocumentData && !editingDraftId) {
        // Only load if not editing a draft
        try {
          console.log("Loading edit document data:", editDocumentData);

          setTitle(editDocumentData.title);
          setTags(editDocumentData.tags);
          setAuthors(editDocumentData.authors);

          // Load message content
          if (editDocumentData.content.message) {
            setMessage(editDocumentData.content.message);
          }

          // Document is freshly loaded, no unsaved changes yet
        } catch (error) {
          console.error("Failed to load edit document data:", error);
        }
      }
    };

    loadEditDocument();
  }, [editDocumentData, editingDraftId]); // React to changes in editDocumentData, but prevent running when editing drafts

  // Fetch the document being replied to
  useEffect(() => {
    if (replyTo) {
      setReplyToLoading(true);
      // Extract document_id from "post_id:document_id" format
      const documentId = parseInt(replyTo.split(":")[1]);
      fetchDocument(documentId)
        .then((doc) => {
          setReplyToDocument(doc.metadata);
        })
        .catch((error) => {
          console.error("Failed to fetch reply-to document:", error);
        })
        .finally(() => {
          setReplyToLoading(false);
        });
    }
  }, [replyTo]);

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

      {/* Reply To Banner */}
      {replyTo && replyToDocument && (
        <div className="px-6 py-2 bg-muted/50 border-b shrink-0">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <MessageSquareIcon className="h-4 w-4" />
            <span>Replying to:</span>
            <span className="font-medium text-foreground">
              {replyToDocument.title}
            </span>
            <span className="text-xs">by {replyToDocument.uploader_id}</span>
          </div>
        </div>
      )}

      {replyTo && replyToLoading && (
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
