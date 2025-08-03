import {
  PlusIcon,
  Trash2Icon,
  XIcon,
  MessageSquareIcon
} from "lucide-react";
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
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../../ui/card";
import { Input } from "../../ui/input";
import { Label } from "../../ui/label";
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
  }, [
    title,
    message,
    tags,
    authors,
    currentDraftId,
    replyTo
  ]);

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
              content_type: 'message' as const,
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
    <Card className="w-full max-w-4xl">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div className="flex-1">
            <div className="flex items-center gap-3">
              <CardTitle className="text-xl">
                {editingDraftId || currentDraftId
                  ? "Edit Document Draft"
                  : replyTo
                    ? `Reply to Document #${replyTo.split(":")[1]} (Post ${replyTo.split(":")[0]})`
                    : "New Document"}
              </CardTitle>
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                {lastSavedAt ? (
                  <span>
                    Last updated{" "}
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
            {replyTo && replyToDocument && (
              <div className="mt-2 p-3 bg-muted rounded-lg">
                <div className="flex items-center gap-2 text-sm text-muted-foreground mb-1">
                  <MessageSquareIcon className="h-4 w-4" />
                  <span>Replying to:</span>
                </div>
                <div className="font-medium text-foreground truncate">
                  {replyToDocument.title}
                </div>
                <div className="text-xs text-muted-foreground mt-1">
                  by u/{replyToDocument.uploader_id}
                </div>
              </div>
            )}
            {replyTo && replyToLoading && (
              <div className="mt-2 p-3 bg-muted rounded-lg">
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <div className="animate-spin rounded-full h-4 w-4 border-b border-current"></div>
                  <span>Loading document details...</span>
                </div>
              </div>
            )}
          </div>
        </div>
      </CardHeader>

      <CardContent className="space-y-6">
        {/* Title Input */}
        <div className="space-y-2">
          <Label htmlFor="title">Title *</Label>
          <Input
            id="title"
            placeholder="Enter a descriptive title for your document"
            value={title}
            onChange={(e) => {
              setTitle(e.target.value);
              setHasUnsavedChangesWithLogging(true);
            }}
            onBlur={() => setTitleTouched(true)}
            maxLength={200}
            className={
              titleTouched && title.trim().length === 0
                ? "border-destructive"
                : ""
            }
          />
          {titleTouched && title.trim().length === 0 && (
            <p className="text-sm text-destructive">Title is required</p>
          )}
          <p className="text-sm text-muted-foreground">
            {title.length}/200 characters
          </p>
        </div>

        {/* Markdown Editor */}
        <div>
          <Label className="text-base font-medium">Content *</Label>
          <div className="mt-2">
            <MarkdownEditor
              value={message}
              onChange={(value) => {
                setMessage(value);
                setHasUnsavedChangesWithLogging(true);
              }}
              placeholder="Enter your markdown content..."
            />
          </div>
        </div>

        {/* Tags */}
        <div className="space-y-2">
          <Label>Tags (optional)</Label>
          <div className="flex gap-2">
            <Input
              placeholder="Add a tag..."
              value={tagInput}
              onChange={(e) => setTagInput(e.target.value)}
              onKeyPress={(e) => handleKeyPress(e, addTag)}
              className="flex-1"
            />
            <Button type="button" variant="outline" size="sm" onClick={addTag}>
              <PlusIcon className="h-4 w-4" />
            </Button>
          </div>
          {tags.length > 0 && (
            <div className="flex flex-wrap gap-2 mt-2">
              {tags.map((tag) => (
                <Badge
                  key={tag}
                  variant="secondary"
                  className="flex items-center gap-1"
                >
                  {tag}
                  <button
                    onClick={() => removeTag(tag)}
                    className="ml-1 hover:text-destructive"
                  >
                    <XIcon className="h-3 w-3" />
                  </button>
                </Badge>
              ))}
            </div>
          )}
        </div>

        {/* Authors */}
        <div className="space-y-2">
          <Label>Authors (optional)</Label>
          <div className="flex gap-2">
            <Input
              placeholder="Add an author..."
              value={authorInput}
              onChange={(e) => setAuthorInput(e.target.value)}
              onKeyPress={(e) => handleKeyPress(e, addAuthor)}
              autoComplete="off"
              className="flex-1"
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={addAuthor}
            >
              <PlusIcon className="h-4 w-4" />
            </Button>
          </div>
          {authors.length > 0 && (
            <div className="flex flex-wrap gap-2 mt-2">
              {authors.map((author) => (
                <Badge
                  key={author}
                  variant="outline"
                  className="flex items-center gap-1"
                >
                  {author}
                  <button
                    onClick={() => removeAuthor(author)}
                    className="ml-1 hover:text-destructive"
                  >
                    <XIcon className="h-3 w-3" />
                  </button>
                </Badge>
              ))}
            </div>
          )}
          <p className="text-sm text-muted-foreground">
            If no authors are specified, you will be listed as the default
            author.
          </p>
        </div>

        {/* Action Buttons */}
        <div className="flex justify-between pt-4 border-t">
          <Button
            variant="outline"
            onClick={deleteDraft}
            className="flex items-center gap-2 text-destructive hover:text-destructive"
          >
            <Trash2Icon className="h-4 w-4" />
            Discard Draft
          </Button>

          <div className="flex gap-3 ml-auto">
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
      </CardContent>
    </Card>
  );
}