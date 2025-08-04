import { MessageSquareIcon, PlusIcon, Trash2Icon, XIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
  createDraft,
  deleteDraft as deleteDraftApi,
  fetchDocument,
  getDraft,
  updateDraft,
  type DocumentMetadata
} from "../../../lib/documentApi";
import { useDocuments } from "../../../lib/store";
import { Button } from "../../ui/button";
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
  const { editDocumentData, setEditDocumentData, navigateToDrafts } =
    useDocuments();

  const [title, setTitle] = useState("");
  const [titleTouched, setTitleTouched] = useState(false);
  const [message, setMessage] = useState("");
  const [tags, setTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState("");
  const [authors, setAuthors] = useState<string[]>([]);
  const [authorInput, setAuthorInput] = useState("");
  const [replyToDocument, setReplyToDocument] =
    useState<DocumentMetadata | null>(null);
  const [replyToLoading, setReplyToLoading] = useState(false);

  // Draft-related state
  const [lastSavedAt, setLastSavedAt] = useState<Date | null>(null);
  const [currentDraftId, setCurrentDraftId] = useState<string | undefined>(
    editingDraftId
  );
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);

  const setHasUnsavedChangesWithLogging = (value: boolean) => {
    setHasUnsavedChanges(value);
  };

  const addTag = () => {
    const trimmedTag = tagInput.trim();
    if (trimmedTag && !tags.includes(trimmedTag)) {
      setTags([...tags, trimmedTag]);
      setTagInput("");
      setHasUnsavedChangesWithLogging(true);
    }
  };

  const removeTag = (tagToRemove: string) => {
    setTags(tags.filter((tag) => tag !== tagToRemove));
    setHasUnsavedChangesWithLogging(true);
  };

  const addAuthor = () => {
    const trimmedAuthor = authorInput.trim();
    if (trimmedAuthor && !authors.includes(trimmedAuthor)) {
      setAuthors([...authors, trimmedAuthor]);
      setAuthorInput("");
      setHasUnsavedChangesWithLogging(true);
    }
  };

  const removeAuthor = (authorToRemove: string) => {
    setAuthors(authors.filter((author) => author !== authorToRemove));
    setHasUnsavedChangesWithLogging(true);
  };

  const handleKeyPress = (e: React.KeyboardEvent, action: () => void) => {
    if (e.key === "Enter") {
      e.preventDefault();
      action();
    }
  };

  const getPublishData = () => {
    console.log("getPublishData called, editDocumentData:", editDocumentData);

    const data: any = {
      title: title.trim(),
      message: message.trim(),
      tags: tags.length > 0 ? tags : undefined,
      authors: authors.length > 0 ? authors : undefined,
      replyTo,
      draftId: currentDraftId || editingDraftId, // Pass the draft ID for deletion after publish
      postId: editDocumentData?.postId // Pass post ID for editing documents (creating revisions)
    };

    console.log("getPublishData result:", data);
    console.log("editDocumentData?.postId:", editDocumentData?.postId);

    return data;
  };

  const handleSubmitAttempt = () => {
    setTitleTouched(true);
  };

  const isValid = () => {
    // Title and message are mandatory for documents
    return title.trim().length > 0 && message.trim().length > 0;
  };

  const deleteDraft = async () => {
    try {
      const draftIdToDelete = currentDraftId || editingDraftId;

      if (draftIdToDelete) {
        const success = await deleteDraftApi(draftIdToDelete);

        if (success) {
          toast.success("Draft discarded");
          setCurrentDraftId(undefined);
          setHasUnsavedChangesWithLogging(false);
        } else {
          toast.error("Failed to discard draft");
          return;
        }
      } else {
        toast.success("Draft discarded");
        setHasUnsavedChangesWithLogging(false);
      }

      // Navigate based on context
      if (editingDraftId) {
        navigateToDrafts();
      } else if (onCancel) {
        onCancel();
      }
    } catch (error) {
      console.error("Failed to delete draft:", error);
      toast.error("Failed to discard draft");
    }
  };

  // Load existing draft if editing
  useEffect(() => {
    const loadDraft = async () => {
      if (editingDraftId) {
        try {
          const draft = await getDraft(editingDraftId);
          if (draft) {
            setTitle(draft.title);
            setMessage(draft.message || "");
            setTags(draft.tags);
            setAuthors(draft.authors);
            setLastSavedAt(new Date(draft.updated_at));
            setHasUnsavedChangesWithLogging(false); // Draft is freshly loaded, no unsaved changes
          }
        } catch (error) {
          console.error("Failed to load draft:", error);
        }
      }
    };

    loadDraft();
  }, [editingDraftId]);

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

          setHasUnsavedChangesWithLogging(false); // Document is freshly loaded, no unsaved changes
        } catch (error) {
          console.error("Failed to load edit document data:", error);
          toast.error("Failed to load document for editing");
        }
      }
    };

    loadEditDocument();
  }, [editDocumentData, editingDraftId]); // React to changes in editDocumentData, but prevent running when editing drafts

  // Use refs to capture current state values for cleanup
  const hasUnsavedChangesRef = useRef(hasUnsavedChanges);
  const currentStateRef = useRef({
    title,
    message,
    tags,
    authors,
    currentDraftId,
    replyTo
  });

  // Update refs when state changes
  useEffect(() => {
    hasUnsavedChangesRef.current = hasUnsavedChanges;
  }, [hasUnsavedChanges]);

  useEffect(() => {
    currentStateRef.current = {
      title,
      message,
      tags,
      authors,
      currentDraftId,
      replyTo
    };
  }, [title, message, tags, authors, currentDraftId, replyTo]);

  // Save draft only when component unmounts (not on every state change)
  useEffect(() => {
    return () => {
      // Use ref values to get current state, not stale closure values
      const currentHasUnsavedChanges = hasUnsavedChangesRef.current;
      const currentState = currentStateRef.current;

      // Check if we have content using current state
      const hasCurrentContent = !!(
        currentState.title.trim() ||
        currentState.message.trim() ||
        currentState.tags.length > 0 ||
        currentState.authors.length > 0
      );

      // Only save if we have unsaved changes and meaningful content
      if (currentHasUnsavedChanges && hasCurrentContent) {
        // Create the draft data directly here since we can't call functions in cleanup
        const saveDraftData = async () => {
          try {
            const draftData = {
              title: currentState.title.trim(),
              content_type: "message" as const,
              message: currentState.message || null,
              file_name: null,
              file_content: null,
              file_mime_type: null,
              url: null,
              tags: currentState.tags,
              authors: currentState.authors,
              reply_to: currentState.replyTo || null
            };

            if (currentState.currentDraftId) {
              await updateDraft(currentState.currentDraftId, draftData);
            } else {
              await createDraft(draftData);
            }
          } catch (error) {
            console.error("Failed to save draft on unmount:", error);
          }
        };

        // Fire and forget
        saveDraftData();
      }
    };
  }, []); // Empty deps - only runs on mount/unmount, safe from React Strict Mode

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
    <div className="h-screen flex flex-col overflow-hidden">
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
              setHasUnsavedChangesWithLogging(true);
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
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">Tags:</span>
          <div className="flex items-center gap-1">
            {tags.map((tag) => (
              <div
                key={tag}
                className="inline-flex items-center gap-1 px-2 py-1 bg-muted rounded-md text-sm"
              >
                <span>{tag}</span>
                <button
                  onClick={() => removeTag(tag)}
                  className="hover:text-destructive"
                >
                  <XIcon className="h-3 w-3" />
                </button>
              </div>
            ))}
            <input
              type="text"
              placeholder={tags.length === 0 ? "Add tags..." : ""}
              value={tagInput}
              onChange={(e) => setTagInput(e.target.value)}
              onKeyPress={(e) => handleKeyPress(e, addTag)}
              autoComplete="off"
              autoCorrect="off"
              className="bg-transparent border-none outline-none text-sm placeholder:text-muted-foreground w-20 min-w-[5rem]"
            />
            <Button type="button" variant="ghost" size="sm" onClick={addTag}>
              <PlusIcon className="h-3 w-3" />
            </Button>
          </div>
        </div>

        {/* Authors Section */}
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">Authors:</span>
          <div className="flex items-center gap-1">
            {authors.map((author) => (
              <div
                key={author}
                className="inline-flex items-center gap-1 px-2 py-1 bg-muted/50 border rounded-md text-sm"
              >
                <span>{author}</span>
                <button
                  onClick={() => removeAuthor(author)}
                  className="hover:text-destructive"
                >
                  <XIcon className="h-3 w-3" />
                </button>
              </div>
            ))}
            <input
              type="text"
              placeholder={authors.length === 0 ? "Add authors..." : ""}
              value={authorInput}
              onChange={(e) => setAuthorInput(e.target.value)}
              onKeyPress={(e) => handleKeyPress(e, addAuthor)}
              autoComplete="off"
              autoCorrect="off"
              className="bg-transparent border-none outline-none text-sm placeholder:text-muted-foreground w-24 min-w-[6rem]"
            />
            <Button type="button" variant="ghost" size="sm" onClick={addAuthor}>
              <PlusIcon className="h-3 w-3" />
            </Button>
          </div>
        </div>

        {/* Status */}
        <div className="text-sm text-muted-foreground shrink-0">
          {lastSavedAt ? (
            <span>
              Saved{" "}
              {lastSavedAt.toLocaleTimeString([], {
                hour: "2-digit",
                minute: "2-digit"
              })}
            </span>
          ) : (
            <span>Draft</span>
          )}
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
            <span className="text-xs">by u/{replyToDocument.uploader_id}</span>
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
            setHasUnsavedChangesWithLogging(true);
          }}
          placeholder="Enter your markdown content..."
          className="h-full"
        />
      </div>

      {/* Bottom Action Bar */}
      <div className="flex items-center justify-between px-6 py-4 border-t bg-background shrink-0">
        <Button
          variant="outline"
          onClick={deleteDraft}
          className="flex items-center gap-2 text-destructive hover:text-destructive"
        >
          <Trash2Icon className="h-4 w-4" />
          Discard Draft
        </Button>

        <PublishButton
          data={getPublishData()}
          disabled={!isValid()}
          onPublishSuccess={(documentId) => {
            console.log("onPublishSuccess called");
            hasUnsavedChangesRef.current = false;
            // Clear edit document data after successful publish
            if (editDocumentData) {
              setEditDocumentData(null);
            }
            // Call the original success callback
            if (onPublishSuccess) {
              onPublishSuccess(documentId);
            }
          }}
          onSubmitAttempt={handleSubmitAttempt}
        />
      </div>
    </div>
  );
}
