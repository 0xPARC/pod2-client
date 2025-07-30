import {
  FileIcon,
  LinkIcon,
  MessageSquareIcon,
  PlusIcon,
  Trash2Icon,
  XIcon
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
  createDraft,
  deleteDraft as deleteDraftApi,
  fetchDocument,
  getDraft,
  updateDraft,
  type DocumentMetadata
} from "../../lib/documentApi";
import { useAppStore } from "../../lib/store";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../ui/tabs";
import { Textarea } from "../ui/textarea";
import { PublishButton } from "./PublishButton";

interface PublishFormProps {
  onPublishSuccess?: (documentId: number) => void;
  onCancel?: () => void;
  replyTo?: string;
  editingDraftId?: string; // UUID
}

export function PublishForm({
  onPublishSuccess,
  onCancel,
  replyTo,
  editingDraftId
}: PublishFormProps) {
  const { setCurrentView, editDocumentData, setEditDocumentData } = useAppStore();
  const [activeTab, setActiveTab] = useState<"message" | "file" | "url">(
    "message"
  );
  const [title, setTitle] = useState("");
  const [titleTouched, setTitleTouched] = useState(false);
  const [message, setMessage] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [url, setUrl] = useState("");
  const [tags, setTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState("");
  const [authors, setAuthors] = useState<string[]>([]);
  const [authorInput, setAuthorInput] = useState("");
  const [isDragOver, setIsDragOver] = useState(false);
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

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);

    const files = Array.from(e.dataTransfer.files);
    if (files.length > 0) {
      setFile(files[0]);
      setActiveTab("file");
      setHasUnsavedChangesWithLogging(true);
    }
  }, []);

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      setFile(files[0]);
      setHasUnsavedChangesWithLogging(true);
    }
  };

  const getPublishData = () => {
    console.log("getPublishData called, editDocumentData:", editDocumentData);
    
    const data: any = {
      title: title.trim(),
      tags: tags.length > 0 ? tags : undefined,
      authors: authors.length > 0 ? authors : undefined,
      replyTo,
      draftId: currentDraftId || editingDraftId, // Pass the draft ID for deletion after publish
      postId: editDocumentData?.postId // Pass post ID for editing documents (creating revisions)
    };

    console.log("getPublishData result:", data);
    console.log("editDocumentData?.postId:", editDocumentData?.postId);

    switch (activeTab) {
      case "message":
        if (message.trim()) {
          data.message = message.trim();
        }
        break;
      case "file":
        if (file) {
          data.file = file;
        }
        break;
      case "url":
        if (url.trim()) {
          data.url = url.trim();
        }
        break;
    }

    return data;
  };

  const handleSubmitAttempt = () => {
    setTitleTouched(true);
  };

  const isValid = () => {
    // Title is mandatory
    if (title.trim().length === 0) {
      return false;
    }

    // At least one content type must be provided
    switch (activeTab) {
      case "message":
        return message.trim().length > 0;
      case "file":
        return file !== null;
      case "url":
        return url.trim().length > 0;
      default:
        return false;
    }
  };

  const formatFileSize = (bytes: number) => {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
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
        setCurrentView("drafts");
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
            setActiveTab(draft.content_type as "message" | "file" | "url");
            setMessage(draft.message || "");
            setUrl(draft.url || "");
            setTags(draft.tags);
            setAuthors(draft.authors);

            // Handle file content if present
            if (draft.file_content && draft.file_name && draft.file_mime_type) {
              const uint8Array = new Uint8Array(draft.file_content);
              const blob = new Blob([uint8Array], {
                type: draft.file_mime_type
              });
              const file = new File([blob], draft.file_name, {
                type: draft.file_mime_type
              });
              setFile(file);
            }

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
      if (editDocumentData && !editingDraftId) { // Only load if not editing a draft
        try {
          console.log("Loading edit document data:", editDocumentData);
          
          setTitle(editDocumentData.title);
          setTags(editDocumentData.tags);
          setAuthors(editDocumentData.authors);

          // Determine content type and set appropriate tab
          if (editDocumentData.content.message) {
            setActiveTab("message");
            setMessage(editDocumentData.content.message);
          } else if (editDocumentData.content.file) {
            setActiveTab("file");
            // Recreate File object from stored data
            const uint8Array = new Uint8Array(editDocumentData.content.file.content);
            const blob = new Blob([uint8Array], {
              type: editDocumentData.content.file.mime_type
            });
            const file = new File([blob], editDocumentData.content.file.name, {
              type: editDocumentData.content.file.mime_type
            });
            setFile(file);
          } else if (editDocumentData.content.url) {
            setActiveTab("url");
            setUrl(editDocumentData.content.url);
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

  // Note: editDocumentData cleanup is handled in the PublishButton onPublishSuccess callback

  // Use refs to capture current state values for cleanup
  const hasUnsavedChangesRef = useRef(hasUnsavedChanges);
  const currentStateRef = useRef({
    title,
    message,
    url,
    file,
    tags,
    authors,
    activeTab,
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
      url,
      file,
      tags,
      authors,
      activeTab,
      currentDraftId,
      replyTo
    };
  }, [
    title,
    message,
    url,
    file,
    tags,
    authors,
    activeTab,
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
        currentState.url.trim() ||
        currentState.file ||
        currentState.tags.length > 0 ||
        currentState.authors.length > 0
      );

      // Only save if we have unsaved changes and meaningful content
      if (currentHasUnsavedChanges && hasCurrentContent) {
        // Create the draft data directly here since we can't call functions in cleanup
        const saveDraftData = async () => {
          try {
            let fileContent = null;
            if (currentState.activeTab === "file" && currentState.file) {
              fileContent = Array.from(
                new Uint8Array(await currentState.file.arrayBuffer())
              );
            }

            const draftData = {
              title: currentState.title.trim(),
              content_type: currentState.activeTab,
              message:
                currentState.activeTab === "message"
                  ? currentState.message || null
                  : null,
              file_name:
                currentState.activeTab === "file" && currentState.file
                  ? currentState.file.name
                  : null,
              file_content: fileContent,
              file_mime_type:
                currentState.activeTab === "file" && currentState.file
                  ? currentState.file.type
                  : null,
              url:
                currentState.activeTab === "url"
                  ? currentState.url || null
                  : null,
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
                  ? "Edit Draft"
                  : replyTo
                    ? `Reply to Document #${replyTo.split(":")[1]} (Post ${replyTo.split(":")[0]})`
                    : "Publish New Document"}
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

        {/* Content Input */}
        <div>
          <Label className="text-base font-medium">Content</Label>
          <Tabs
            value={activeTab}
            onValueChange={(value) => {
              setActiveTab(value as any);
              setHasUnsavedChangesWithLogging(true);
            }}
            className="mt-2"
          >
            <TabsList className="grid w-full grid-cols-3">
              <TabsTrigger value="message" className="flex items-center gap-2">
                <MessageSquareIcon className="h-4 w-4" />
                Message
              </TabsTrigger>
              <TabsTrigger value="file" className="flex items-center gap-2">
                <FileIcon className="h-4 w-4" />
                File
              </TabsTrigger>
              <TabsTrigger value="url" className="flex items-center gap-2">
                <LinkIcon className="h-4 w-4" />
                URL
              </TabsTrigger>
            </TabsList>

            <TabsContent value="message" className="mt-4">
              <Textarea
                placeholder="Enter your message content (supports Markdown)..."
                value={message}
                onChange={(e) => {
                  setMessage(e.target.value);
                  setHasUnsavedChangesWithLogging(true);
                }}
                className="min-h-[200px] resize-none"
              />
              <p className="text-sm text-muted-foreground mt-2">
                Supports Markdown formatting including **bold**, *italic*,
                `code`, and more.
              </p>
            </TabsContent>

            <TabsContent value="file" className="mt-4">
              <div
                className={`border-2 border-dashed rounded-lg p-8 text-center transition-colors ${
                  isDragOver
                    ? "border-primary bg-primary/5"
                    : "border-muted-foreground/25 hover:border-muted-foreground/50"
                }`}
                onDragOver={handleDragOver}
                onDragLeave={handleDragLeave}
                onDrop={handleDrop}
              >
                {file ? (
                  <div className="space-y-4">
                    <div className="flex items-center justify-center gap-3">
                      <FileIcon className="h-8 w-8 text-primary" />
                      <div className="text-left">
                        <p className="font-medium">{file.name}</p>
                        <p className="text-sm text-muted-foreground">
                          {formatFileSize(file.size)} •{" "}
                          {file.type || "Unknown type"}
                        </p>
                      </div>
                    </div>
                    <div className="flex gap-2">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          setFile(null);
                          setHasUnsavedChangesWithLogging(true);
                        }}
                      >
                        Remove File
                      </Button>
                      <label htmlFor="file-input">
                        <Button variant="outline" size="sm" asChild>
                          <span>Choose Different File</span>
                        </Button>
                      </label>
                    </div>
                  </div>
                ) : (
                  <div className="space-y-4">
                    <FileIcon className="h-12 w-12 mx-auto text-muted-foreground" />
                    <div>
                      <p className="text-lg font-medium">Drop a file here</p>
                      <p className="text-muted-foreground">
                        or click to browse
                      </p>
                    </div>
                    <label htmlFor="file-input">
                      <Button variant="outline" asChild>
                        <span>Choose File</span>
                      </Button>
                    </label>
                  </div>
                )}
                <input
                  id="file-input"
                  type="file"
                  className="hidden"
                  onChange={handleFileSelect}
                />
              </div>
            </TabsContent>

            <TabsContent value="url" className="mt-4">
              <Input
                placeholder="https://example.com/document"
                value={url}
                onChange={(e) => {
                  setUrl(e.target.value);
                  setHasUnsavedChangesWithLogging(true);
                }}
                type="url"
              />
              <p className="text-sm text-muted-foreground mt-2">
                Reference a URL that contains the document content.
              </p>
            </TabsContent>
          </Tabs>
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
