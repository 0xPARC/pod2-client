import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import "highlight.js/styles/github-dark.css";
import {
  ArrowLeftIcon,
  FileTextIcon,
  CheckCircleIcon,
  AlertCircleIcon,
  ClockIcon,
  UserIcon,
  TagIcon,
  ExternalLinkIcon,
  DownloadIcon
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { Button } from "../ui/button";
import { Badge } from "../ui/badge";
import { Separator } from "../ui/separator";
import { fetchDocument, Document, DocumentMetadata } from "../../lib/documentApi";

interface DocumentDetailViewProps {
  documentId: number;
  onBack: () => void;
}

export function DocumentDetailView({
  documentId,
  onBack
}: DocumentDetailViewProps) {
  const [document, setDocument] = useState<Document | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [verificationStatus, setVerificationStatus] = useState<{
    publishVerified: boolean;
    timestampVerified: boolean;
    upvoteCountVerified: boolean;
    isVerifying: boolean;
  }>({
    publishVerified: false,
    timestampVerified: false,
    upvoteCountVerified: false,
    isVerifying: false
  });

  const loadDocument = async () => {
    try {
      setLoading(true);
      setError(null);
      const doc = await fetchDocument(documentId);
      setDocument(doc);

      // Start verification process
      verifyDocument(doc);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load document");
    } finally {
      setLoading(false);
    }
  };

  const verifyDocument = async (doc: Document) => {
    setVerificationStatus((prev) => ({ ...prev, isVerifying: true }));

    // TODO: Implement actual verification calls to Tauri commands
    // For now, simulate verification with a delay
    setTimeout(() => {
      setVerificationStatus({
        publishVerified: true,
        timestampVerified: true,
        upvoteCountVerified: true,
        isVerifying: false
      });
    }, 1500);
  };

  useEffect(() => {
    loadDocument();
  }, [documentId]);

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

  const renderContent = (content: Document["content"]) => {
    if (!content.message) return null;

    // Check if content looks like markdown
    const isMarkdown =
      content.message.includes("#") ||
      content.message.includes("**") ||
      content.message.includes("*") ||
      content.message.includes("```") ||
      (content.message.includes("[") && content.message.includes("](")) ||
      content.message.includes("- ") ||
      content.message.includes("1. ");
    console.log("is markdown?: ", isMarkdown);

    if (isMarkdown) {
      return (
        <div className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            rehypePlugins={[rehypeHighlight]}
            components={{
              // Custom link component to handle external links safely
              a: ({ href, children, ...props }) => (
                <a
                  href={href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-blue-600 hover:text-blue-800 underline"
                  {...props}
                >
                  {children}
                </a>
              ),
              // Custom code block styling
              pre: ({ children, ...props }) => (
                <pre
                  className="bg-muted border rounded-lg p-4 overflow-x-auto"
                  {...props}
                >
                  {children}
                </pre>
              ),
              // Custom table styling
              table: ({ children, ...props }) => (
                <div className="overflow-x-auto">
                  <table
                    className="min-w-full border-collapse border border-border"
                    {...props}
                  >
                    {children}
                  </table>
                </div>
              ),
              th: ({ children, ...props }) => (
                <th
                  className="border border-border bg-muted px-4 py-2 text-left font-medium"
                  {...props}
                >
                  {children}
                </th>
              ),
              td: ({ children, ...props }) => (
                <td className="border border-border px-4 py-2" {...props}>
                  {children}
                </td>
              )
            }}
          >
            {content.message}
          </ReactMarkdown>
        </div>
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
      console.log("📁 File content preview:", fileContent.substring(0, 100) + "...");
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
              <Button variant="outline" size="sm">
                <DownloadIcon className="h-4 w-4 mr-2" />
                Download
              </Button>
            </div>
          </div>

          <div className="prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none border rounded-lg p-6 max-h-[70vh] overflow-y-auto">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={[rehypeHighlight]}
              components={{
                // Custom link component to handle external links safely
                a: ({ href, children, ...props }) => (
                  <a
                    href={href}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-blue-600 hover:text-blue-800 underline"
                    {...props}
                  >
                    {children}
                  </a>
                ),
                // Custom code block styling
                pre: ({ children, ...props }) => (
                  <pre
                    className="bg-muted border rounded-lg p-4 overflow-x-auto"
                    {...props}
                  >
                    {children}
                  </pre>
                ),
                // Custom table styling
                table: ({ children, ...props }) => (
                  <div className="overflow-x-auto">
                    <table
                      className="min-w-full border-collapse border border-border"
                      {...props}
                    >
                      {children}
                    </table>
                  </div>
                ),
                th: ({ children, ...props }) => (
                  <th
                    className="border border-border bg-muted px-4 py-2 text-left font-medium"
                    {...props}
                  >
                    {children}
                  </th>
                ),
                td: ({ children, ...props }) => (
                  <td className="border border-border px-4 py-2" {...props}>
                    {children}
                  </td>
                )
              }}
            >
              {fileContent}
            </ReactMarkdown>
          </div>
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
              <Button variant="outline" size="sm">
                <DownloadIcon className="h-4 w-4 mr-2" />
                Download
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
            <Button variant="outline" size="sm">
              <DownloadIcon className="h-4 w-4 mr-2" />
              Download
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
          <Button variant="ghost" onClick={onBack} className="mb-4">
            <ArrowLeftIcon className="h-4 w-4 mr-2" />
            Back to Documents
          </Button>
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
          <Button variant="ghost" onClick={onBack} className="mb-4">
            <ArrowLeftIcon className="h-4 w-4 mr-2" />
            Back to Documents
          </Button>
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
        <Button variant="ghost" onClick={onBack} className="mb-4">
          <ArrowLeftIcon className="h-4 w-4 mr-2" />
          Back to Documents
        </Button>

        {/* Document Header */}
        <Card className="mb-6">
          <CardHeader>
            <div className="flex items-start justify-between">
              <div>
                <CardTitle className="text-2xl flex items-center gap-2">
                  <FileTextIcon className="h-6 w-6" />
                  Document #{document.metadata.id}
                </CardTitle>
                <p className="text-muted-foreground mt-1">
                  Post {document.metadata.post_id} • Revision{" "}
                  {document.metadata.revision}
                </p>
              </div>

              <div className="flex items-center gap-2">
                {verificationStatus.isVerifying ? (
                  <Badge variant="outline">
                    <div className="animate-spin rounded-full h-3 w-3 border-b border-current mr-1"></div>
                    Verifying...
                  </Badge>
                ) : (
                  <Badge
                    variant={
                      verificationStatus.publishVerified &&
                      verificationStatus.timestampVerified &&
                      verificationStatus.upvoteCountVerified
                        ? "default"
                        : "destructive"
                    }
                  >
                    <CheckCircleIcon className="h-3 w-3 mr-1" />
                    {verificationStatus.publishVerified &&
                    verificationStatus.timestampVerified &&
                    verificationStatus.upvoteCountVerified
                      ? "Verified"
                      : "Verification Failed"}
                  </Badge>
                )}

                {document.metadata.upvote_count > 0 && (
                  <Badge variant="secondary">
                    {document.metadata.upvote_count} upvote
                    {document.metadata.upvote_count !== 1 ? "s" : ""}
                  </Badge>
                )}
              </div>
            </div>
          </CardHeader>

          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
              <div className="flex items-center gap-2">
                <UserIcon className="h-4 w-4" />
                <span className="font-medium">Uploader:</span>
                <span>{document.metadata.uploader_id}</span>
              </div>

              <div className="flex items-center gap-2">
                <ClockIcon className="h-4 w-4" />
                <span className="font-medium">Created:</span>
                <span>{formatDate(document.metadata.created_at)}</span>
              </div>

              <div className="min-w-0">
                <span className="font-medium">Content ID:</span>
                <code className="ml-1 text-xs bg-muted px-1 py-0.5 rounded break-all">
                  {document.metadata.content_id.slice(0, 16)}...
                </code>
              </div>
            </div>

            {(document.metadata.tags.length > 0 ||
              document.metadata.authors.length > 0) && (
              <>
                <Separator className="my-4" />
                <div className="space-y-3">
                  {document.metadata.tags.length > 0 && (
                    <div className="flex items-center gap-2 flex-wrap">
                      <TagIcon className="h-4 w-4" />
                      <span className="font-medium">Tags:</span>
                      {document.metadata.tags.map((tag, index) => (
                        <Badge
                          key={index}
                          variant="outline"
                          className="text-xs"
                        >
                          {tag}
                        </Badge>
                      ))}
                    </div>
                  )}

                  {document.metadata.authors.length > 0 && (
                    <div className="flex items-center gap-2 flex-wrap">
                      <UserIcon className="h-4 w-4" />
                      <span className="font-medium">Authors:</span>
                      {document.metadata.authors.map((author, index) => (
                        <Badge
                          key={index}
                          variant="secondary"
                          className="text-xs"
                        >
                          {author}
                        </Badge>
                      ))}
                    </div>
                  )}
                </div>
              </>
            )}
          </CardContent>
        </Card>

        {/* Document Content */}
        <Card>
          <CardHeader>
            <CardTitle>Content</CardTitle>
          </CardHeader>
          <CardContent>
            {/* Render message content if it exists */}
            {document.content.message && renderContent(document.content)}

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
                <div className="text-center py-8 text-muted-foreground">
                  <FileTextIcon className="h-12 w-12 mx-auto mb-2" />
                  <p>Content not available or unsupported format</p>
                </div>
              )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

