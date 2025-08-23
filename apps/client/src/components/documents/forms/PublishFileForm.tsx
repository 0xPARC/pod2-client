import { useEffect, useState } from "react";
import type { Author } from "../../../lib/documentApi";
import { useDocuments } from "../../../lib/store";
import { Button } from "../../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../../ui/card";
import { ChipInput } from "../../ui/chip-input";
import { Input } from "../../ui/input";
import { Label } from "../../ui/label";
import { PublishButton } from "../PublishButton";
import { FileEditor } from "../editors/FileEditor";
import { AuthorSelector } from "./AuthorSelector";

interface PublishFileFormProps {
  onPublishSuccess?: (documentId: number) => void;
  onCancel?: () => void;
}

export function PublishFileForm({
  onPublishSuccess,
  onCancel
}: PublishFileFormProps) {
  const { currentRoute } = useDocuments();

  // Get route-specific edit document data
  const editDocumentData = currentRoute?.editDocumentData;

  const [title, setTitle] = useState("");
  const [titleTouched, setTitleTouched] = useState(false);
  const [file, setFile] = useState<File | null>(null);
  const [tags, setTags] = useState<string[]>([]);
  const [authors, setAuthors] = useState<Author[]>([]);
  const [isEditMode, setIsEditMode] = useState(false);

  const getPublishData = () => {
    return {
      title: title.trim(),
      file: file || undefined,
      tags: tags.length > 0 ? tags : undefined,
      authors:
        authors.length > 0
          ? authors.map((a) =>
              a.author_type === "github" ? a.github_username : a.username
            )
          : undefined,
      postId: editDocumentData?.postId // Pass post ID for editing documents (creating revisions)
    };
  };

  const handleSubmitAttempt = () => {
    setTitleTouched(true);
  };

  const isValid = () => {
    // Title is always required
    // File is required for new documents, but optional when editing (keep existing file)
    return title.trim().length > 0 && (file !== null || isEditMode);
  };

  const handleCancel = () => {
    if (onCancel) {
      onCancel();
    }
  };

  // Load existing document data for editing
  useEffect(() => {
    if (editDocumentData) {
      console.log("Loading file edit document data:", editDocumentData);
      setIsEditMode(true);

      setTitle(editDocumentData.title);
      setTags(editDocumentData.tags);
      setAuthors(editDocumentData.authors);

      // For file editing, we don't pre-populate the file since it requires
      // the user to select a new file or keep the existing one
      // The existing file information is available in editDocumentData.content.file
    }
  }, [editDocumentData]);

  return (
    <Card className="w-full max-w-4xl">
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="text-xl">
            {isEditMode ? "Edit File Document" : "Upload File"}
          </CardTitle>
        </div>
        {isEditMode && editDocumentData?.content.file && (
          <div className="text-sm text-muted-foreground">
            Currently editing:{" "}
            <span className="font-medium">
              {editDocumentData.content.file.name}
            </span>{" "}
            ({editDocumentData.content.file.mime_type})
            <br />
            Upload a new file to replace it, or leave empty to keep the existing
            file.
          </div>
        )}
      </CardHeader>

      <CardContent className="space-y-6">
        {/* Title Input */}
        <div className="space-y-2">
          <Label htmlFor="title">Title *</Label>
          <Input
            id="title"
            placeholder="Enter a descriptive title for the file"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
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

        {/* File Editor */}
        <div>
          <Label className="text-base font-medium">
            File {isEditMode ? "(optional - upload new file to replace)" : "*"}
          </Label>
          <div className="mt-2">
            <FileEditor file={file} onFileChange={setFile} />
          </div>
        </div>

        {/* Tags */}
        <ChipInput
          label="Tags (optional)"
          placeholder="Add a tag..."
          values={tags}
          onValuesChange={setTags}
          variant="secondary"
        />

        {/* Authors */}
        <AuthorSelector
          label="Authors (optional)"
          value={authors}
          onChange={setAuthors}
        />

        {/* Action Buttons */}
        <div className="flex justify-between pt-4 border-t">
          <Button variant="outline" onClick={handleCancel}>
            Cancel
          </Button>

          <PublishButton
            data={getPublishData()}
            disabled={!isValid()}
            onPublishSuccess={onPublishSuccess}
            onSubmitAttempt={handleSubmitAttempt}
          />
        </div>
      </CardContent>
    </Card>
  );
}
