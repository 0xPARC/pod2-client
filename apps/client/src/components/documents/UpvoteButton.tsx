import { DEFAULT_SERVER_URL } from "@/lib/documentApi";
import { invoke } from "@tauri-apps/api/core";
import { PlusIcon } from "lucide-react";
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

function UpvoteLoadingToast() {
  return (
    <div className="flex items-center">
      <span>Generating upvote POD</span>
      <AnimatedDots />
    </div>
  );
}

interface UpvoteButtonProps {
  documentId: number;
  currentUpvotes: number;
  disabled?: boolean;
  onUpvoteSuccess?: (newCount: number) => void;
}

export function UpvoteButton({
  documentId,
  currentUpvotes,
  disabled = false,
  onUpvoteSuccess
}: UpvoteButtonProps) {
  const [isLoading, setIsLoading] = useState(false);

  const handleUpvote = async () => {
    if (isLoading || disabled) return;

    setIsLoading(true);

    // Show loading toast with animated component
    const loadingToast = toast(<UpvoteLoadingToast />, {
      duration: Infinity // Keep it open until we dismiss it
    });

    try {
      // Get server URL from environment or use default
      const serverUrl = DEFAULT_SERVER_URL;

      // Call the Tauri upvote command
      const result = await invoke<{
        success: boolean;
        new_upvote_count: number | null;
        error_message: string | null;
        already_upvoted: boolean;
      }>("upvote_document", {
        documentId: documentId,
        serverUrl: serverUrl
      });

      // Dismiss loading toast
      toast.dismiss(loadingToast);

      if (result.success && result.new_upvote_count !== null) {
        // Success - update count and show success message
        toast.success("Document upvoted successfully!");

        if (onUpvoteSuccess) {
          onUpvoteSuccess(result.new_upvote_count);
        }
      } else if (result.already_upvoted) {
        // Already upvoted
        toast.info("You have already upvoted this document");
      } else {
        // Other error
        toast.error(result.error_message || "Failed to upvote document");
      }
    } catch (error) {
      // Dismiss loading toast and show error
      toast.dismiss(loadingToast);
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      toast.error(`Failed to upvote document: ${errorMessage}`);
      console.error("Upvote error:", error);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Button
      variant="outline"
      size="sm"
      onClick={handleUpvote}
      disabled={isLoading || disabled}
      className="flex items-center gap-1 min-w-0"
    >
      <PlusIcon className={`h-3 w-3 ${isLoading ? "opacity-50" : ""}`} />
      <span className="text-xs">
        {currentUpvotes} upvote{currentUpvotes !== 1 ? "s" : ""}
      </span>
    </Button>
  );
}
