import {
  FileIcon,
  FileTextIcon,
  ImageIcon,
  UploadIcon,
  XIcon
} from "lucide-react";
import { useCallback, useState } from "react";
import { Button } from "../../ui/button";
import { Card, CardContent } from "../../ui/card";

interface FileEditorProps {
  file: File | null;
  onFileChange: (file: File | null) => void;
  className?: string;
}

export function FileEditor({ file, onFileChange, className }: FileEditorProps) {
  const [isDragOver, setIsDragOver] = useState(false);

  // Handle drag and drop
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setIsDragOver(false);

      const files = Array.from(e.dataTransfer.files);
      if (files.length > 0) {
        onFileChange(files[0]);
      }
    },
    [onFileChange]
  );

  // Handle file input
  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      onFileChange(files[0]);
    }
  };

  // Remove file
  const handleRemoveFile = () => {
    onFileChange(null);
  };

  // Get file type icon
  const getFileIcon = (file: File) => {
    const type = file.type.toLowerCase();
    if (type.startsWith("image/")) {
      return <ImageIcon className="w-8 h-8" />;
    } else if (
      type.includes("text/") ||
      type.includes("pdf") ||
      type.includes("document")
    ) {
      return <FileTextIcon className="w-8 h-8" />;
    } else {
      return <FileIcon className="w-8 h-8" />;
    }
  };

  // Format file size
  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  };

  // Create file preview for images
  const createImagePreview = (file: File): string => {
    return URL.createObjectURL(file);
  };

  return (
    <div className={`space-y-4 ${className}`}>
      {!file ? (
        // File upload area
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
          <div className="flex flex-col items-center gap-4">
            <UploadIcon className="w-12 h-12 text-muted-foreground" />
            <div>
              <p className="text-lg font-medium">Drop a file here</p>
              <p className="text-sm text-muted-foreground mt-1">
                or click to browse your files
              </p>
            </div>
            <input
              type="file"
              id="file-upload"
              className="hidden"
              onChange={handleFileSelect}
            />
            <Button
              variant="outline"
              onClick={() => document.getElementById("file-upload")?.click()}
            >
              Choose File
            </Button>
          </div>
        </div>
      ) : (
        // File preview
        <Card>
          <CardContent className="p-4">
            <div className="flex items-start gap-4">
              {/* File icon or image preview */}
              <div className="flex-shrink-0">
                {file.type.startsWith("image/") ? (
                  <img
                    src={createImagePreview(file)}
                    alt="File preview"
                    className="w-16 h-16 object-cover rounded border"
                    onError={(e) => {
                      // Fallback to file icon if image fails to load
                      e.currentTarget.replaceWith(
                        Object.assign(document.createElement("div"), {
                          className:
                            "w-16 h-16 flex items-center justify-center border rounded bg-muted",
                          innerHTML:
                            '<svg class="w-8 h-8"><use href="#file-icon"/></svg>'
                        })
                      );
                    }}
                  />
                ) : (
                  <div className="w-16 h-16 flex items-center justify-center border rounded bg-muted">
                    {getFileIcon(file)}
                  </div>
                )}
              </div>

              {/* File details */}
              <div className="flex-1 min-w-0">
                <h3 className="font-medium truncate" title={file.name}>
                  {file.name}
                </h3>
                <p className="text-sm text-muted-foreground mt-1">
                  {formatFileSize(file.size)}
                </p>
                {file.type && (
                  <p className="text-xs text-muted-foreground mt-1">
                    {file.type}
                  </p>
                )}
              </div>

              {/* Actions */}
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleRemoveFile}
                  title="Remove file"
                >
                  <XIcon className="w-4 h-4" />
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* File constraints info */}
      <div className="text-sm text-muted-foreground space-y-1">
        <p>Supported formats: Images, documents, text files, and more</p>
        <p>Maximum file size: 10 MB</p>
      </div>
    </div>
  );
}
