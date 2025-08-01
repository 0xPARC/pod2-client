import { invoke } from "@tauri-apps/api/core";
import { SendIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "../ui/button";

function AnimatedDots() {
  const [dotCount, setDotCount] = useState(1);

  useEffect(() => {
    const interval = setInterval(() => {
      setDotCount((prev) => (prev >= 4 ? 1 : prev + 1));
    }, 350);

    return () => clearInterval(interval);
  }, []);

  return <span>{".".repeat(dotCount)}</span>;
}

function PublishLoadingToast({ isEditing }: { isEditing: boolean }) {
  return (
    <div className="flex items-center">
      <span>{isEditing ? "Updating document" : "Publishing document"}</span>
      <AnimatedDots />
    </div>
  );
}

interface PublishData {
  title: string;
  message?: string;
  file?: File;
  url?: string;
  tags?: string[];
  authors?: string[];
  replyTo?: string;
  draftId?: string; // UUID of draft to delete after successful publish
  postId?: number; // Optional post ID for creating revisions (editing documents)
}

interface PublishButtonProps {
  data: PublishData;
  disabled?: boolean;
  onPublishSuccess?: (documentId: number) => void;
  onSubmitAttempt?: () => void;
  variant?: "default" | "outline";
  size?: "default" | "sm" | "lg";
  className?: string;
}

export function PublishButton({
  data,
  disabled = false,
  onPublishSuccess,
  onSubmitAttempt,
  variant = "default",
  size = "default",
  className = ""
}: PublishButtonProps) {
  const [isLoading, setIsLoading] = useState(false);

  const handlePublish = async () => {
    if (isLoading || disabled) return;

    // Call the submit attempt callback to show validation
    onSubmitAttempt?.();

    // Validate that we have a title
    if (!data.title || data.title.trim().length === 0) {
      toast.error("Please provide a title for your document");
      return;
    }

    // Validate that we have at least one content type
    if (!data.message && !data.file && !data.url) {
      toast.error("Please provide message, file, or URL content");
      return;
    }

    setIsLoading(true);

    // Show loading toast with animated component
    const loadingToast = toast(
      <PublishLoadingToast isEditing={!!data.postId} />,
      {
        duration: Infinity // Keep it open until we dismiss it
      }
    );

    try {
      // Get server URL from configuration
      const networkConfig = await invoke<any>("get_config_section", {
        section: "network"
      });
      const serverUrl = networkConfig.document_server;

      // Prepare file data if file is provided
      let fileData = null;
      if (data.file) {
        const fileContent = await data.file.arrayBuffer();
        fileData = {
          name: data.file.name,
          content: Array.from(new Uint8Array(fileContent)),
          mime_type: data.file.type || "application/octet-stream"
        };
      }

      console.log("Publishing with data:", data);
      console.log("Reply to being sent (raw):", data.replyTo);
      console.log("Post ID for editing:", data.postId);

      const invokeParams = {
        title: data.title.trim(),
        message: data.message || null,
        file: fileData,
        url: data.url || null,
        tags: data.tags || [],
        authors: data.authors || [],
        replyTo: data.replyTo
          ? {
              post_id: parseInt(data.replyTo.split(":")[0]),
              document_id: parseInt(data.replyTo.split(":")[1])
            }
          : null,
        serverUrl: serverUrl,
        draftId: data.draftId || null,
        postId: data.postId || null
      };
      console.log("Full invoke parameters:", invokeParams);

      // Call the Tauri publish command
      const result = await invoke<{
        success: boolean;
        document_id: number | null;
        error_message: string | null;
      }>("publish_document", invokeParams);

      // Dismiss loading toast
      toast.dismiss(loadingToast);

      if (result.success && result.document_id !== null) {
        // Success - show success message and callback
        toast.success(
          data.postId
            ? "Document updated successfully!"
            : "Document published successfully!"
        );

        if (onPublishSuccess) {
          onPublishSuccess(result.document_id);
        }
      } else {
        // Error
        toast.error(result.error_message || "Failed to publish document");
      }
    } catch (error) {
      // Dismiss loading toast and show error
      toast.dismiss(loadingToast);
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      toast.error(`Failed to publish document: ${errorMessage}`);
      console.error("Publish error:", error);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Button
      variant={variant}
      size={size}
      onClick={handlePublish}
      disabled={isLoading || disabled}
      className={`flex items-center gap-2 ${className}`}
    >
      <SendIcon className={`h-4 w-4 ${isLoading ? "opacity-50" : ""}`} />
      <span>
        {isLoading
          ? data.postId
            ? "Updating..."
            : "Publishing..."
          : data.postId
            ? "Update Document"
            : "Publish Document"}
      </span>
    </Button>
  );
}
