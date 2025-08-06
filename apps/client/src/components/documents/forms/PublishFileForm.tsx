import { useState } from "react";
import { Button } from "../../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../../ui/card";
import { ChipInput } from "../../ui/chip-input";
import { Input } from "../../ui/input";
import { Label } from "../../ui/label";
import { PublishButton } from "../PublishButton";
import { FileEditor } from "../editors/FileEditor";

interface PublishFileFormProps {
  onPublishSuccess?: (documentId: number) => void;
  onCancel?: () => void;
}

export function PublishFileForm({
  onPublishSuccess,
  onCancel
}: PublishFileFormProps) {
  const [title, setTitle] = useState("");
  const [titleTouched, setTitleTouched] = useState(false);
  const [file, setFile] = useState<File | null>(null);
  const [tags, setTags] = useState<string[]>([]);
  const [authors, setAuthors] = useState<string[]>([]);

  const getPublishData = () => {
    return {
      title: title.trim(),
      file: file || undefined,
      tags: tags.length > 0 ? tags : undefined,
      authors: authors.length > 0 ? authors : undefined
    };
  };

  const handleSubmitAttempt = () => {
    setTitleTouched(true);
  };

  const isValid = () => {
    // Title and file are mandatory for file uploads
    return title.trim().length > 0 && file !== null;
  };

  const handleCancel = () => {
    if (onCancel) {
      onCancel();
    }
  };

  return (
    <Card className="w-full max-w-4xl">
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="text-xl">Upload File</CardTitle>
        </div>
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
          <Label className="text-base font-medium">File *</Label>
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
