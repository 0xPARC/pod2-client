import { useEffect, useState } from "react";
import { Button } from "../../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../../ui/card";
import { ChipInput } from "../../ui/chip-input";
import { Input } from "../../ui/input";
import { Label } from "../../ui/label";
import { PublishButton } from "../PublishButton";
import { FileEditor } from "../editors/FileEditor";
import type { EditingDocument } from "../PublishPage";

interface PublishFileFormProps {
  onPublishSuccess?: (documentId: number) => void;
  onCancel?: () => void;
  editingDocument?: EditingDocument; // For editing existing documents
}

export function PublishFileForm({
  onPublishSuccess,
  onCancel,
  editingDocument
}: PublishFileFormProps) {
  const isEditMode = !!editingDocument;

  const [title, setTitle] = useState(editingDocument?.title ?? "");
  const [titleTouched, setTitleTouched] = useState(false);
  const [file, setFile] = useState<File | null>(null); // File editing will allow replacement
  const [tags, setTags] = useState<string[]>(editingDocument?.tags ?? []);
  const [authors, setAuthors] = useState<string[]>(
    editingDocument?.authors ?? []
  );

  const getPublishData = () => {
    return {
      title: title.trim(),
      file: file || undefined,
      tags: tags.length > 0 ? tags : undefined,
      authors: authors.length > 0 ? authors : undefined,
      replyTo: editingDocument?.replyTo ?? undefined,
      postId: editingDocument?.postId // Include postId when editing existing document
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

  // Initialize existing file info when editing
  useEffect(() => {
    if (isEditMode && editingDocument?.content.file?.name) {
      // For editing, we show the existing file name but allow replacement
      console.log("Editing file:", editingDocument.content.file.name);
    }
  }, [isEditMode, editingDocument]);

  return (
    <Card className="w-full max-w-4xl">
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="text-xl">
            {isEditMode ? "Edit File" : "Upload File"}
          </CardTitle>
        </div>
        {isEditMode && editingDocument?.content.file?.name && (
          <p className="text-sm text-muted-foreground">
            Editing: {editingDocument.content.file.name}
          </p>
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
            File{" "}
            {isEditMode ? "(optional - leave blank to keep existing)" : "*"}
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
        <ChipInput
          label="Authors (optional)"
          placeholder="Add an author..."
          values={authors}
          onValuesChange={setAuthors}
          variant="outline"
          helpText="If no authors are specified, you will be listed as the default author."
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
