import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import "highlight.js/styles/github-dark.css";
import {
  AlertCircleIcon,
  CheckCircleIcon,
  DownloadIcon,
  EditIcon,
  ExternalLinkIcon,
  FileTextIcon,
  MessageSquareIcon,
  ReplyIcon,
  TrashIcon
} from "lucide-react";
import { useEffect, useState, useMemo } from "react";
import { toast } from "sonner";
import {
  Document,
  DocumentFile,
  DocumentMetadata,
  DocumentVerificationResult,
  fetchDocument,
  fetchDocumentReplies,
  fetchPostReplies,
  verifyDocumentPod,
  deleteDocument,
  getCurrentUsername
} from "../../lib/documentApi";
import { useDocuments } from "../../lib/store";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { useMarkdownRenderer, renderMarkdownToHtml } from "./markdownRenderer";

// Interface for threaded reply structure
interface ThreadedReply extends DocumentMetadata {
  children: ThreadedReply[];
  depth: number;
}

interface DocumentDetailViewProps {
  documentId: number;
  onNavigateToDocument?: (documentId: number) => void;
}

// MIME type to file extension mapping
const getMimeTypeExtension = (mimeType: string): string => {
  const mimeToExt: Record<string, string> = {
    // Text types
    "text/plain": "txt",
    "text/markdown": "md",
    "text/html": "html",
    "text/css": "css",
    "text/javascript": "js",
    "text/csv": "csv",

    // Image types
    "image/jpeg": "jpg",
    "image/png": "png",
    "image/gif": "gif",
    "image/svg+xml": "svg",
    "image/webp": "webp",

    // Document types
    "application/pdf": "pdf",
    "application/json": "json",
    "application/xml": "xml",
    "application/zip": "zip",

    // Microsoft Office
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document":
      "docx",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": "xlsx",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation":
      "pptx",

    // Audio/Video
    "audio/mpeg": "mp3",
    "video/mp4": "mp4",
    "video/webm": "webm"
  };

  return mimeToExt[mimeType] || "bin";
};

// Get file filters for save dialog based on MIME type
const getFileFilters = (mimeType: string) => {
  const ext = getMimeTypeExtension(mimeType);
  const baseFilters = [
    {
      name: "All Files",
      extensions: ["*"]
    }
  ];

  if (ext !== "bin") {
    baseFilters.unshift({
      name: `${ext.toUpperCase()} Files`,
      extensions: [ext]
    });
  }

  return baseFilters;
};

// Ensure filename has proper extension
const ensureFileExtension = (filename: string, mimeType: string): string => {
  const ext = getMimeTypeExtension(mimeType);
  if (ext === "bin") return filename;

  const hasExtension =
    filename.includes(".") && filename.split(".").pop()?.toLowerCase() === ext;
  return hasExtension ? filename : `${filename}.${ext}`;
};

export function DocumentDetailView({
  documentId,
  onNavigateToDocument
}: DocumentDetailViewProps) {
  const { setEditDocumentData, navigateToPublish, navigateToDocumentsList } =
    useDocuments();
  const [document, setDocument] = useState<Document | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [verificationResult, setVerificationResult] =
    useState<DocumentVerificationResult | null>(null);
  const [isVerifying, setIsVerifying] = useState(false);
  const [verificationError, setVerificationError] = useState<string | null>(
    null
  );
  const [upvoteCount, setUpvoteCount] = useState<number>(0);
  const [isUpvoting, setIsUpvoting] = useState(false);
  const [downloadingFiles, setDownloadingFiles] = useState<Set<string>>(
    new Set()
  );
  const [replies, setReplies] = useState<DocumentMetadata[]>([]);
  const [repliesLoading, setRepliesLoading] = useState(false);
  const [repliesError, setRepliesError] = useState<string | null>(null);
  const [currentUsername, setCurrentUsername] = useState<string | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  // Use shared markdown renderer
  const md = useMarkdownRenderer();

  const loadDocument = async () => {
    try {
      setLoading(true);
      setError(null);
      const doc = await fetchDocument(documentId);
      setDocument(doc);
      setUpvoteCount(doc.metadata.upvote_count);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load document");
    } finally {
      setLoading(false);
    }
  };

  // Recursively fetch all replies to build complete conversation tree
  const fetchAllRepliesRecursively = async (
    postId: number,
    visited: Set<number> = new Set()
  ): Promise<DocumentMetadata[]> => {
    // Get direct replies to this post
    const directReplies = await fetchPostReplies(postId);
    const allReplies: DocumentMetadata[] = [...directReplies];

    // For each direct reply, recursively fetch its replies
    for (const reply of directReplies) {
      if (!reply.id || visited.has(reply.id)) {
        continue;
      }

      visited.add(reply.id);

      try {
        // Fetch replies to this specific document
        const nestedReplies = await fetchDocumentReplies(reply.id);

        if (nestedReplies.length > 0) {
          allReplies.push(...nestedReplies);

          // Recursively fetch replies to nested replies
          for (const nestedReply of nestedReplies) {
            if (nestedReply.id && !visited.has(nestedReply.id)) {
              visited.add(nestedReply.id);
              const deeperReplies = await fetchDocumentReplies(nestedReply.id);
              allReplies.push(...deeperReplies);
            }
          }
        }
      } catch (error) {
        console.warn(
          `Failed to fetch replies for document ${reply.id}:`,
          error
        );
        // Continue with other replies even if one fails
      }
    }

    return allReplies;
  };

  const loadReplies = async () => {
    if (!documentId || !document) return;

    try {
      setRepliesLoading(true);
      setRepliesError(null);

      // Use recursive fetching to get complete conversation tree
      const allRepliesData = await fetchAllRepliesRecursively(
        document.metadata.post_id
      );
      setReplies(allRepliesData);
    } catch (err) {
      // Fallback to basic post replies if recursive fails
      try {
        console.warn(
          "Recursive replies failed, falling back to basic post replies:",
          err
        );
        const postRepliesData = await fetchPostReplies(
          document.metadata.post_id
        );
        setReplies(postRepliesData);
      } catch (fallbackErr) {
        console.error("Both recursive and basic replies failed:", fallbackErr);
        setRepliesError(
          fallbackErr instanceof Error
            ? fallbackErr.message
            : "Failed to load replies"
        );
      }
    } finally {
      setRepliesLoading(false);
    }
  };

  const handleVerifyDocument = async () => {
    if (!document) return;

    try {
      setIsVerifying(true);
      setVerificationError(null);
      const result = await verifyDocumentPod(document);
      console.log("Verification result:", result);
      setVerificationResult(result);
    } catch (err) {
      setVerificationError(
        err instanceof Error ? err.message : "Failed to verify document"
      );
    } finally {
      setIsVerifying(false);
    }
  };

  const handleReplyToDocument = () => {
    if (!document) return;

    // Format: "post_id:document_id"
    const replyToId = `${document.metadata.post_id}:${document.metadata.id}`;
    console.log("Navigating to reply with replyTo:", replyToId);
    // Navigate to publish page with reply context
    navigateToPublish(undefined, "document", replyToId);
  };

  const handleUpvote = async () => {
    if (isUpvoting || !document) return;

    setIsUpvoting(true);

    // Show loading toast
    const loadingToast = toast("Generating upvote POD...", {
      duration: Infinity
    });

    try {
      const networkConfig = await invoke<any>("get_config_section", {
        section: "network"
      });
      const serverUrl = networkConfig.document_server;

      // Call the Tauri upvote command
      const result = await invoke<{
        success: boolean;
        new_upvote_count: number | null;
        error_message: string | null;
        already_upvoted: boolean;
      }>("upvote_document", {
        documentId: document.metadata.id,
        serverUrl: serverUrl
      });

      // Dismiss loading toast
      toast.dismiss(loadingToast);

      if (result.success && result.new_upvote_count !== null) {
        // Success - update count and show success message
        toast.success("Document upvoted successfully!");
        setUpvoteCount(result.new_upvote_count);
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
      setIsUpvoting(false);
    }
  };

  const handleDeleteDocument = async () => {
    if (!document || isDeleting) return;

    // Confirm deletion
    if (
      !confirm(
        "Are you sure you want to delete this document? This action cannot be undone."
      )
    ) {
      return;
    }

    setIsDeleting(true);

    // Show loading toast
    const loadingToast = toast("Deleting document...", {
      duration: Infinity
    });

    try {
      const networkConfig = await invoke<any>("get_config_section", {
        section: "network"
      });
      const serverUrl = networkConfig.document_server;

      // Call the Tauri delete command
      const result = await deleteDocument(document.metadata.id!, serverUrl);

      // Dismiss loading toast
      toast.dismiss(loadingToast);

      if (result.success) {
        toast.success("Document deleted successfully!");
        // Navigate back to documents list
        navigateToDocumentsList();
      } else {
        toast.error(result.error_message || "Failed to delete document");
      }
    } catch (error) {
      // Dismiss loading toast
      toast.dismiss(loadingToast);

      const errorMessage =
        error instanceof Error ? error.message : "Failed to delete document";
      console.error("Delete error:", error);
      toast.error(errorMessage);
    } finally {
      setIsDeleting(false);
    }
  };

  const handleEditDocument = () => {
    if (!document) return;

    // Set the document data for editing in the store
    setEditDocumentData({
      documentId: document.metadata.id!,
      postId: document.metadata.post_id,
      title: document.metadata.title || "",
      content: document.content,
      tags: document.metadata.tags,
      authors: document.metadata.authors,
      replyTo: document.metadata.reply_to
        ? `${document.metadata.reply_to.post_id}:${document.metadata.reply_to.document_id}`
        : null
    });

    // Navigate to publish view in edit mode
    navigateToPublish();
  };

  useEffect(() => {
    loadDocument();
  }, [documentId]);

  useEffect(() => {
    const loadCurrentUsername = async () => {
      try {
        const username = await getCurrentUsername();
        setCurrentUsername(username);
      } catch (error) {
        console.error("Failed to get current username:", error);
      }
    };
    loadCurrentUsername();
  }, []);

  useEffect(() => {
    if (document) {
      loadReplies();
    }
  }, [document]);

  const handleDownloadFile = async (file: DocumentFile) => {
    const fileKey = `${file.name}_${file.mime_type}`;

    if (downloadingFiles.has(fileKey)) {
      return; // Already downloading
    }

    try {
      setDownloadingFiles((prev) => new Set(prev).add(fileKey));

      // Ensure filename has proper extension
      const filename = ensureFileExtension(file.name, file.mime_type);

      // Show save dialog
      const filePath = await save({
        defaultPath: filename,
        filters: getFileFilters(file.mime_type)
      });

      if (!filePath) {
        // User cancelled the save dialog
        return;
      }

      // Convert file content from number array to Uint8Array
      const fileContent = new Uint8Array(file.content);

      // Write file to chosen location
      await writeFile(filePath, fileContent);

      toast.success(`File "${filename}" saved successfully!`);
    } catch (error) {
      console.error("Download error:", error);
      const errorMessage =
        error instanceof Error ? error.message : "Unknown error";
      toast.error(`Failed to save file: ${errorMessage}`);
    } finally {
      setDownloadingFiles((prev) => {
        const newSet = new Set(prev);
        newSet.delete(fileKey);
        return newSet;
      });
    }
  };

  const formatDate = (dateString?: string) => {
    if (!dateString) return "Unknown";
    return new Date(dateString).toLocaleDateString(undefined, {
      year: "numeric",
      month: "long",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit"
    });
  };

  // Build threaded reply tree from flat list
  const buildReplyTree = (replies: DocumentMetadata[]): ThreadedReply[] => {
    const replyMap = new Map<number, ThreadedReply>();
    const rootReplies: ThreadedReply[] = [];

    // First pass: create all reply objects
    replies.forEach((reply) => {
      const threadedReply: ThreadedReply = {
        ...reply,
        children: [],
        depth: 0
      };
      replyMap.set(reply.id!, threadedReply);
    });

    // Second pass: build parent-child relationships
    replies.forEach((reply) => {
      const threadedReply = replyMap.get(reply.id!)!;

      if (reply.reply_to?.document_id) {
        // This is a reply to another document
        const parentReply = replyMap.get(reply.reply_to.document_id);
        if (parentReply) {
          // It's a reply to another reply
          threadedReply.depth = parentReply.depth + 1;
          parentReply.children.push(threadedReply);
        } else {
          // It's a reply to the original document (not in replies list)
          rootReplies.push(threadedReply);
        }
      } else {
        // Top-level reply
        rootReplies.push(threadedReply);
      }
    });

    return rootReplies;
  };

  // Recursive component to render a reply and its children
  const renderThreadedReply = (reply: ThreadedReply): React.JSX.Element => {
    const isReplyToCurrentDoc = reply.reply_to?.document_id === documentId;
    const isReplyToCurrentPost =
      reply.reply_to?.post_id === document?.metadata.post_id;
    const maxDepth = 5; // Limit nesting depth for readability
    const displayDepth = Math.min(reply.depth, maxDepth);

    return (
      <div key={reply.id} className="space-y-4">
        <div
          className={`border-l-2 border-muted pl-4 ${
            displayDepth > 0 ? `ml-${Math.min(displayDepth * 4, 16)}` : ""
          }`}
          style={{
            marginLeft: displayDepth > 0 ? `${displayDepth * 16}px` : "0"
          }}
        >
          <div className="flex items-start justify-between mb-2">
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span className="font-medium text-blue-600">
                u/{reply.uploader_id}
              </span>
              <span>•</span>
              <span>{formatDate(reply.created_at)}</span>
              <span>•</span>
              <span className="text-orange-600">
                #{reply.id} ({reply.upvote_count} upvotes)
              </span>
              {displayDepth > 0 && (
                <>
                  <span>•</span>
                  <span className="text-xs text-muted-foreground bg-muted px-2 py-1 rounded">
                    Level {displayDepth}
                  </span>
                </>
              )}
            </div>
          </div>

          {/* Show which version this reply targets */}
          {reply.reply_to && (
            <div className="mb-2">
              {isReplyToCurrentDoc ? (
                <Badge
                  variant="outline"
                  className="text-xs bg-green-50 text-green-700 border-green-200"
                >
                  Reply to this version
                </Badge>
              ) : isReplyToCurrentPost ? (
                <div className="flex items-center gap-2">
                  <Badge
                    variant="outline"
                    className="text-xs bg-yellow-50 text-yellow-700 border-yellow-200"
                  >
                    Reply to different version
                  </Badge>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      onNavigateToDocument?.(reply.reply_to!.document_id)
                    }
                    className="text-yellow-700 hover:text-yellow-900 p-0 h-auto font-normal text-xs underline"
                    disabled={!onNavigateToDocument}
                  >
                    (view version #{reply.reply_to.document_id})
                  </Button>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  <Badge
                    variant="outline"
                    className="text-xs bg-blue-50 text-blue-700 border-blue-200"
                  >
                    Reply to #{reply.reply_to.document_id}
                  </Badge>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      onNavigateToDocument?.(reply.reply_to!.document_id)
                    }
                    className="text-blue-700 hover:text-blue-900 p-0 h-auto font-normal text-xs underline"
                    disabled={!onNavigateToDocument}
                  >
                    (view doc #{reply.reply_to.document_id})
                  </Button>
                </div>
              )}
            </div>
          )}

          <h4 className="font-medium text-foreground mb-2">{reply.title}</h4>

          {reply.tags.length > 0 && (
            <div className="flex gap-1 mb-2">
              {reply.tags.map((tag, index) => (
                <Badge key={index} variant="outline" className="text-xs">
                  {tag}
                </Badge>
              ))}
            </div>
          )}

          {reply.authors.length > 0 && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground mb-2">
              <span>Authors:</span>
              {reply.authors.map((author, index) => (
                <Badge key={index} variant="secondary" className="text-xs">
                  {author}
                </Badge>
              ))}
            </div>
          )}

          <Button
            variant="ghost"
            size="sm"
            onClick={() => onNavigateToDocument?.(reply.id!)}
            className="text-blue-600 hover:text-blue-800 p-0 h-auto font-normal"
            disabled={!onNavigateToDocument}
          >
            View full reply →
          </Button>
        </div>

        {/* Render children recursively */}
        {reply.children.length > 0 && (
          <div className="space-y-4">
            {reply.children.map((child) => renderThreadedReply(child))}
          </div>
        )}
      </div>
    );
  };

  // Memoize rendered HTML for message content
  const renderedMessageHtml = useMemo(() => {
    if (!document?.content.message) return null;

    // Check if content looks like markdown
    const isMarkdown =
      document.content.message.includes("#") ||
      document.content.message.includes("**") ||
      document.content.message.includes("*") ||
      document.content.message.includes("```") ||
      (document.content.message.includes("[") &&
        document.content.message.includes("](")) ||
      document.content.message.includes("- ") ||
      document.content.message.includes("1. ");

    return {
      html: isMarkdown
        ? renderMarkdownToHtml(md, document.content.message)
        : null,
      isMarkdown
    };
  }, [document?.content.message, md]);

  const renderContent = (content: Document["content"]) => {
    if (!content.message) return null;

    if (renderedMessageHtml?.isMarkdown) {
      return (
        <div
          className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere"
          dangerouslySetInnerHTML={{ __html: renderedMessageHtml.html! }}
        />
      );
    } else {
      return (
        <div className="prose prose-neutral max-w-none dark:prose-invert">
          <p className="whitespace-pre-wrap">{content.message}</p>
        </div>
      );
    }
  };

  const renderFileAttachment = (file: Document["content"]["file"]) => {
    if (!file) return null;

    const isImage = file.mime_type.startsWith("image/");
    const isMarkdown =
      file.mime_type === "text/markdown" ||
      file.name.toLowerCase().endsWith(".md") ||
      file.name.toLowerCase().endsWith(".markdown");
    const isText = file.mime_type.startsWith("text/");

    // For markdown files, render the content directly
    if (
      isMarkdown ||
      (isText &&
        (file.name.toLowerCase().endsWith(".md") ||
          file.name.toLowerCase().endsWith(".markdown")))
    ) {
      // Convert byte array to string properly
      const fileContent = String.fromCharCode(...file.content);
      console.log("📁 File detected as markdown:", file.name);
      console.log("📁 File MIME type:", file.mime_type);
      console.log("📁 File content length:", file.content.length);
      console.log(
        "📁 File content preview:",
        fileContent.substring(0, 100) + "..."
      );
      console.log("📁 isMarkdown flag:", isMarkdown);
      console.log("📁 isText flag:", isText);

      return (
        <div className="mt-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-medium flex items-center gap-2">
              <FileTextIcon className="h-5 w-5" />
              {file.name}
            </h3>
            <div className="flex items-center gap-2">
              <Badge variant="outline" className="text-xs">
                {file.mime_type} • {(file.content.length / 1024).toFixed(1)} KB
              </Badge>
              <Button
                variant="outline"
                size="sm"
                onClick={() => handleDownloadFile(file)}
                disabled={downloadingFiles.has(
                  `${file.name}_${file.mime_type}`
                )}
              >
                <DownloadIcon className="h-4 w-4 mr-2" />
                {downloadingFiles.has(`${file.name}_${file.mime_type}`)
                  ? "Downloading..."
                  : "Download"}
              </Button>
            </div>
          </div>

          <div
            className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere border rounded-lg p-6 max-h-[70vh] overflow-y-auto"
            dangerouslySetInnerHTML={{
              __html: renderMarkdownToHtml(md, fileContent)
            }}
          />
        </div>
      );
    }

    // For other text files, show as plain text
    if (isText && !isMarkdown) {
      const fileContent = String.fromCharCode(...file.content);

      return (
        <div className="mt-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-medium flex items-center gap-2">
              <FileTextIcon className="h-5 w-5" />
              {file.name}
            </h3>
            <div className="flex items-center gap-2">
              <Badge variant="outline" className="text-xs">
                {file.mime_type} • {(file.content.length / 1024).toFixed(1)} KB
              </Badge>
              <Button
                variant="outline"
                size="sm"
                onClick={() => handleDownloadFile(file)}
                disabled={downloadingFiles.has(
                  `${file.name}_${file.mime_type}`
                )}
              >
                <DownloadIcon className="h-4 w-4 mr-2" />
                {downloadingFiles.has(`${file.name}_${file.mime_type}`)
                  ? "Downloading..."
                  : "Download"}
              </Button>
            </div>
          </div>

          <div className="border rounded-lg p-6 bg-muted/20 max-h-[70vh] overflow-y-auto">
            <pre className="whitespace-pre-wrap font-mono text-sm overflow-x-auto">
              {fileContent}
            </pre>
          </div>
        </div>
      );
    }

    // For non-text files (images, binaries, etc.), show as attachment
    return (
      <Card className="mt-4">
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            <FileTextIcon className="h-5 w-5" />
            File Attachment
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div>
              <p className="font-medium">{file.name}</p>
              <p className="text-sm text-muted-foreground">
                {file.mime_type} • {(file.content.length / 1024).toFixed(1)} KB
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => handleDownloadFile(file)}
              disabled={downloadingFiles.has(`${file.name}_${file.mime_type}`)}
            >
              <DownloadIcon
                className={`h-4 w-4 mr-2 ${downloadingFiles.has(`${file.name}_${file.mime_type}`) ? "animate-spin" : ""}`}
              />
              {downloadingFiles.has(`${file.name}_${file.mime_type}`)
                ? "Downloading..."
                : "Download"}
            </Button>
          </div>

          {isImage && (
            <div className="mt-4">
              <img
                src={`data:${file.mime_type};base64,${btoa(String.fromCharCode(...file.content))}`}
                alt={file.name}
                className="max-w-full h-auto rounded-lg border"
              />
            </div>
          )}
        </CardContent>
      </Card>
    );
  };

  const renderUrl = (url: string) => (
    <Card className="mt-4">
      <CardContent className="pt-6">
        <div className="flex items-center gap-2">
          <ExternalLinkIcon className="h-4 w-4" />
          <span className="font-medium">Referenced URL:</span>
          <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-blue-600 hover:text-blue-800 underline"
          >
            {url}
          </a>
        </div>
      </CardContent>
    </Card>
  );

  if (loading) {
    return (
      <div className="p-6 min-h-screen w-full">
        <div className="w-full">
          <Card>
            <CardContent className="pt-6">
              <div className="flex items-center justify-center py-12">
                <div className="text-center">
                  <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-4"></div>
                  Loading document...
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }

  if (error || !document) {
    return (
      <div className="p-6 min-h-screen w-full">
        <div className="w-full">
          <Card className="border-destructive">
            <CardContent className="pt-6">
              <div className="flex items-center gap-2 text-destructive">
                <AlertCircleIcon className="h-5 w-5" />
                <span>{error || "Document not found"}</span>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 min-h-screen w-full overflow-y-auto">
      <div className="w-full">
        {/* Document Header - Reddit-style */}
        <div className="mb-6">
          <div className="flex items-start gap-4">
            {/* Upvote section */}
            <div className="flex flex-col items-center min-w-[80px] pt-2">
              <button
                onClick={handleUpvote}
                disabled={isUpvoting}
                className={`text-3xl mb-2 transition-colors ${
                  isUpvoting
                    ? "text-muted-foreground cursor-not-allowed"
                    : "text-muted-foreground hover:text-orange-500 cursor-pointer"
                }`}
                title="Upvote this document"
              >
                ▲
              </button>
              <div className="text-2xl font-bold text-orange-500 mb-1">
                {upvoteCount}
              </div>
              <div className="text-xs text-muted-foreground">upvotes</div>
            </div>

            {/* Main content header */}
            <div className="flex-1 min-w-0">
              {/* Title */}
              <h1 className="text-3xl font-bold text-foreground mb-4 line-clamp-3">
                {document.metadata.title}
              </h1>

              {/* Author/Uploader info */}
              <div className="flex items-center gap-2 text-sm text-muted-foreground mb-4">
                <span>Posted by</span>
                <span className="font-medium text-blue-600">
                  u/{document.metadata.uploader_id}
                </span>
                <span>•</span>
                <span>{formatDate(document.metadata.created_at)}</span>
                {document.metadata.reply_to && (
                  <>
                    <span>•</span>
                    <span className="text-orange-600">
                      Reply to #{document.metadata.reply_to.document_id} (Post{" "}
                      {document.metadata.reply_to.post_id})
                    </span>
                  </>
                )}
              </div>

              {/* Authors (if different from uploader) */}
              {document.metadata.authors.length > 0 && (
                <div className="flex items-center gap-2 text-sm text-muted-foreground mb-4">
                  <span>Authors:</span>
                  <div className="flex gap-2">
                    {document.metadata.authors.map((author, index) => (
                      <Badge
                        key={index}
                        variant="secondary"
                        className="text-xs bg-blue-100 text-blue-800"
                      >
                        {author}
                      </Badge>
                    ))}
                  </div>
                </div>
              )}

              {/* Tags */}
              {document.metadata.tags.length > 0 && (
                <div className="flex items-center gap-2 mb-6">
                  {document.metadata.tags.map((tag, index) => (
                    <Badge
                      key={index}
                      variant="outline"
                      className="text-xs bg-green-50 text-green-700 border-green-200 hover:bg-green-100"
                    >
                      {tag}
                    </Badge>
                  ))}
                </div>
              )}
            </div>

            {/* Right side - Action buttons */}
            <div className="flex flex-col items-end gap-2">
              <div className="flex items-center gap-2">
                <Button
                  onClick={handleReplyToDocument}
                  variant="outline"
                  size="sm"
                >
                  <ReplyIcon className="h-3 w-3 mr-1" />
                  Reply
                </Button>
                <Button
                  onClick={handleVerifyDocument}
                  disabled={isVerifying}
                  variant="outline"
                  size="sm"
                >
                  {isVerifying ? (
                    <>
                      <div className="animate-spin rounded-full h-3 w-3 border-b border-current mr-1"></div>
                      Verifying...
                    </>
                  ) : (
                    <>
                      <CheckCircleIcon className="h-3 w-3 mr-1" />
                      Verify POD
                    </>
                  )}
                </Button>
                {/* Edit button - only show for document owner */}
                {currentUsername &&
                  document.metadata.uploader_id === currentUsername && (
                    <Button
                      onClick={handleEditDocument}
                      variant="outline"
                      size="sm"
                      className="text-blue-600 hover:text-blue-700 hover:bg-blue-50 border-blue-200"
                    >
                      <EditIcon className="h-3 w-3 mr-1" />
                      Edit
                    </Button>
                  )}
                {/* Delete button - only show for document owner */}
                {currentUsername &&
                  document.metadata.uploader_id === currentUsername && (
                    <Button
                      onClick={handleDeleteDocument}
                      disabled={isDeleting}
                      variant="outline"
                      size="sm"
                      className="text-red-600 hover:text-red-700 hover:bg-red-50 border-red-200"
                    >
                      {isDeleting ? (
                        <>
                          <div className="animate-spin rounded-full h-3 w-3 border-b border-current mr-1"></div>
                          Deleting...
                        </>
                      ) : (
                        <>
                          <TrashIcon className="h-3 w-3 mr-1" />
                          Delete
                        </>
                      )}
                    </Button>
                  )}
              </div>

              {verificationResult &&
                verificationResult.publish_verified &&
                verificationResult.timestamp_verified &&
                verificationResult.upvote_count_verified && (
                  <div className="flex items-center gap-1 text-green-600 text-xs">
                    <CheckCircleIcon className="h-4 w-4" />
                    <span>Verified</span>
                  </div>
                )}
            </div>
          </div>
        </div>

        {/* Show verification error if it exists */}
        {verificationError && (
          <Card className="mb-6 border-destructive">
            <CardContent className="pt-6">
              <div className="flex items-center gap-2 text-destructive">
                <AlertCircleIcon className="h-4 w-4" />
                <span className="font-medium">
                  Verification failed: {verificationError}
                </span>
              </div>
            </CardContent>
          </Card>
        )}

        {/* Document Content - Main Focus */}
        <div className="mb-8">
          {/* Render message content if it exists */}
          {document.content.message && (
            <div className="bg-white dark:bg-gray-900 rounded-lg border p-6 mb-6">
              {renderContent(document.content)}
            </div>
          )}

          {/* If no message content but there's a file, render the file content */}
          {!document.content.message &&
            document.content.file &&
            renderFileAttachment(document.content.file)}

          {/* If there's both message and file, render file as attachment */}
          {document.content.message &&
            document.content.file &&
            renderFileAttachment(document.content.file)}

          {/* Render URL if it exists */}
          {document.content.url && renderUrl(document.content.url)}

          {/* Show empty state only if no content at all */}
          {!document.content.message &&
            !document.content.file &&
            !document.content.url && (
              <div className="text-center py-8 text-muted-foreground bg-muted/30 rounded-lg">
                <FileTextIcon className="h-12 w-12 mx-auto mb-2" />
                <p>Content not available or unsupported format</p>
              </div>
            )}
        </div>

        {/* Replies Section */}
        <Card className="mb-8">
          <CardHeader>
            <CardTitle className="text-lg flex items-center gap-2">
              <MessageSquareIcon className="h-5 w-5" />
              Replies to Post #{document.metadata.post_id} ({replies.length})
            </CardTitle>
            <p className="text-sm text-muted-foreground">
              Showing replies to all versions of this post
            </p>
          </CardHeader>
          <CardContent>
            {repliesLoading && (
              <div className="flex items-center justify-center py-8">
                <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary mr-2"></div>
                Loading replies...
              </div>
            )}

            {repliesError && (
              <div className="flex items-center gap-2 text-destructive py-4">
                <AlertCircleIcon className="h-4 w-4" />
                <span>Failed to load replies: {repliesError}</span>
              </div>
            )}

            {!repliesLoading && !repliesError && replies.length === 0 && (
              <div className="text-center py-8 text-muted-foreground">
                <MessageSquareIcon className="h-12 w-12 mx-auto mb-2 opacity-50" />
                <p>No replies yet</p>
              </div>
            )}

            {!repliesLoading && !repliesError && replies.length > 0 && (
              <div className="space-y-4">
                {buildReplyTree(replies).map((reply) =>
                  renderThreadedReply(reply)
                )}
              </div>
            )}
          </CardContent>
        </Card>

        {/* Technical Details - Moved to Bottom */}
        <Card className="bg-muted/30">
          <CardHeader>
            <CardTitle className="text-lg text-muted-foreground">
              Document Metadata
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 text-sm">
              <div>
                <span className="font-medium text-muted-foreground">
                  Document ID:
                </span>
                <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1">
                  #{document.metadata.id}
                </div>
              </div>

              <div>
                <span className="font-medium text-muted-foreground">
                  Post ID:
                </span>
                <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1">
                  #{document.metadata.post_id}
                </div>
              </div>

              <div>
                <span className="font-medium text-muted-foreground">
                  Revision:
                </span>
                <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1">
                  r{document.metadata.revision}
                </div>
              </div>

              <div className="lg:col-span-2">
                <span className="font-medium text-muted-foreground">
                  Content ID:
                </span>
                <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1 break-all">
                  {document.metadata.content_id}
                </div>
              </div>

              <div>
                <span className="font-medium text-muted-foreground">
                  Verified Upvotes:
                </span>
                <div className="text-xs font-mono bg-muted px-2 py-1 rounded mt-1">
                  {document.metadata.upvote_count}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
