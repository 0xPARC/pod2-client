import {
  FileIcon,
  LinkIcon,
  MessageSquareIcon,
  PlusIcon,
  XIcon
} from "lucide-react";
import { useCallback, useState } from "react";
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
  replyTo?: number;
}

export function PublishForm({
  onPublishSuccess,
  onCancel,
  replyTo
}: PublishFormProps) {
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

  const addTag = () => {
    const trimmedTag = tagInput.trim();
    if (trimmedTag && !tags.includes(trimmedTag)) {
      setTags([...tags, trimmedTag]);
      setTagInput("");
    }
  };

  const removeTag = (tagToRemove: string) => {
    setTags(tags.filter((tag) => tag !== tagToRemove));
  };

  const addAuthor = () => {
    const trimmedAuthor = authorInput.trim();
    if (trimmedAuthor && !authors.includes(trimmedAuthor)) {
      setAuthors([...authors, trimmedAuthor]);
      setAuthorInput("");
    }
  };

  const removeAuthor = (authorToRemove: string) => {
    setAuthors(authors.filter((author) => author !== authorToRemove));
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
    }
  }, []);

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      setFile(files[0]);
    }
  };

  const getPublishData = () => {
    const data: any = {
      title: title.trim(),
      tags: tags.length > 0 ? tags : undefined,
      authors: authors.length > 0 ? authors : undefined,
      replyTo
    };

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

  return (
    <Card className="w-full max-w-4xl">
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="text-xl">
            {replyTo ? `Reply to Document #${replyTo}` : "Publish New Document"}
          </CardTitle>
          {onCancel && (
            <Button variant="ghost" size="sm" onClick={onCancel}>
              <XIcon className="h-4 w-4" />
            </Button>
          )}
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
            onValueChange={(value) => setActiveTab(value as any)}
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
                onChange={(e) => setMessage(e.target.value)}
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
                        onClick={() => setFile(null)}
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
                onChange={(e) => setUrl(e.target.value)}
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
        <div className="flex justify-end gap-3 pt-4 border-t">
          {onCancel && (
            <Button variant="outline" onClick={onCancel}>
              Cancel
            </Button>
          )}
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
