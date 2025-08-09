import { useState } from "react";
import { AlertCircleIcon, CheckIcon, LoaderIcon } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "../../ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "../../ui/dialog";
import { Input } from "../../ui/input";
import { Label } from "../../ui/label";
import { createDraft, DraftRequest } from "../../../lib/documentApi";

interface HackMDImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImportSuccess: (draftId: string) => void;
}

type ImportState = "idle" | "loading" | "success" | "error";

export function HackMDImportDialog({
  open,
  onOpenChange,
  onImportSuccess
}: HackMDImportDialogProps) {
  const [url, setUrl] = useState("");
  const [state, setState] = useState<ImportState>("idle");
  const [error, setError] = useState<string | null>(null);

  const resetState = () => {
    setUrl("");
    setState("idle");
    setError(null);
  };

  const handleOpenChange = (newOpen: boolean) => {
    if (!newOpen) {
      resetState();
    }
    onOpenChange(newOpen);
  };

  // Normalize HackMD URL to markdown format
  const normalizeHackMDUrl = (inputUrl: string): string | null => {
    try {
      const trimmedUrl = inputUrl.trim();

      // Check if it's a HackMD URL
      if (!trimmedUrl.includes("hackmd.io")) {
        return null;
      }

      // Parse URL to handle different formats
      const url = new URL(trimmedUrl);
      if (url.hostname !== "hackmd.io") {
        return null;
      }

      // If already ends with .md, use as-is
      if (url.pathname.endsWith(".md")) {
        return url.toString();
      }

      // Append .md to the path
      const normalizedPath = url.pathname + ".md";
      return `${url.protocol}//${url.hostname}${normalizedPath}${url.search}${url.hash}`;
    } catch {
      return null;
    }
  };

  // Extract title and clean content from markdown, handling YAML frontmatter
  const extractTitleAndCleanContent = (
    content: string
  ): { title: string; cleanContent: string } => {
    const lines = content.split("\n");
    let startIndex = 0;
    let title = "Imported from HackMD";

    // Check for YAML frontmatter
    if (lines[0]?.trim() === "---") {
      // Find the end of frontmatter
      for (let i = 1; i < lines.length; i++) {
        if (lines[i]?.trim() === "---") {
          // Parse frontmatter for title
          const frontmatterLines = lines.slice(1, i);
          for (const line of frontmatterLines) {
            const titleMatch = line.match(/^title:\s*(.+)$/i);
            if (titleMatch) {
              title = titleMatch[1].trim().replace(/^["']|["']$/g, ""); // Remove quotes if present
            }
          }
          // Skip the frontmatter entirely in the cleaned content
          startIndex = i + 1;
          break;
        }
      }
    }

    // If no title found in frontmatter, look for first H1 heading after frontmatter
    if (title === "Imported from HackMD") {
      for (let i = startIndex; i < lines.length; i++) {
        const line = lines[i];
        const h1Match = line.match(/^#\s+(.+)$/);
        if (h1Match) {
          title = h1Match[1].trim();
          break;
        }
      }
    }

    // If still no title, fallback to first non-empty line after frontmatter (truncated)
    if (title === "Imported from HackMD") {
      for (let i = startIndex; i < lines.length; i++) {
        const line = lines[i];
        const trimmed = line.trim();
        if (trimmed && !trimmed.startsWith("#") && !trimmed.startsWith("---")) {
          title =
            trimmed.length > 50 ? trimmed.substring(0, 50) + "..." : trimmed;
          break;
        }
      }
    }

    // Create clean content without frontmatter
    const cleanContent = lines.slice(startIndex).join("\n").trim();

    return { title, cleanContent };
  };

  // Fetch markdown content from HackMD using Tauri
  const fetchHackMDContent = async (normalizedUrl: string): Promise<string> => {
    try {
      return await invoke<string>("fetch_url_text", { url: normalizedUrl });
    } catch (error) {
      if (typeof error === "string") {
        throw new Error(error);
      }
      throw new Error("Failed to fetch document");
    }
  };

  // Create draft from HackMD content
  const createDraftFromContent = async (content: string): Promise<string> => {
    const { title, cleanContent } = extractTitleAndCleanContent(content);

    const draftRequest: DraftRequest = {
      title,
      content_type: "message",
      message: cleanContent,
      file_name: null,
      file_content: null,
      file_mime_type: null,
      url: null,
      tags: ["hackmd", "imported"],
      authors: [],
      reply_to: null
    };

    return await createDraft(draftRequest);
  };

  const handleImport = async () => {
    if (!url.trim()) {
      setError("Please enter a HackMD URL");
      return;
    }

    setState("loading");
    setError(null);

    try {
      // Normalize URL
      const normalizedUrl = normalizeHackMDUrl(url);
      if (!normalizedUrl) {
        throw new Error(
          "Invalid HackMD URL. Please enter a valid HackMD.io URL."
        );
      }

      // Fetch content
      const content = await fetchHackMDContent(normalizedUrl);

      // Create draft
      const draftId = await createDraftFromContent(content);

      setState("success");

      // Call success callback after a brief delay to show success state
      setTimeout(() => {
        onImportSuccess(draftId);
      }, 800);
    } catch (error) {
      setState("error");
      setError(
        error instanceof Error ? error.message : "Failed to import document"
      );
    }
  };

  const isValidUrl = (inputUrl: string): boolean => {
    return normalizeHackMDUrl(inputUrl) !== null;
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Import from HackMD</DialogTitle>
          <DialogDescription>
            Paste a HackMD URL to import the document as a draft.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="hackmd-url">HackMD URL</Label>
            <Input
              id="hackmd-url"
              placeholder="https://hackmd.io/abc123"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              disabled={state === "loading" || state === "success"}
            />
            {url.trim() && !isValidUrl(url) && (
              <p className="text-sm text-destructive">
                Please enter a valid HackMD.io URL
              </p>
            )}
          </div>

          {error && (
            <div className="flex items-center gap-2 p-3 bg-destructive/10 border border-destructive/20 rounded-md">
              <AlertCircleIcon className="h-4 w-4 text-destructive" />
              <p className="text-sm text-destructive">{error}</p>
            </div>
          )}

          {state === "success" && (
            <div className="flex items-center gap-2 p-3 bg-green-50 border border-green-200 rounded-md">
              <CheckIcon className="h-4 w-4 text-green-600" />
              <p className="text-sm text-green-800">
                Document imported successfully!
              </p>
            </div>
          )}
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => handleOpenChange(false)}
            disabled={state === "loading"}
          >
            Cancel
          </Button>
          <Button
            onClick={handleImport}
            disabled={
              !url.trim() ||
              !isValidUrl(url) ||
              state === "loading" ||
              state === "success"
            }
          >
            {state === "loading" && (
              <LoaderIcon className="h-4 w-4 mr-2 animate-spin" />
            )}
            {state === "loading" ? "Importing..." : "Import"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
