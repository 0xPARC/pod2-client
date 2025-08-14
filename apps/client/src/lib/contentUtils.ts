import { Document } from "./documentApi";

// Detect if content looks like markdown
export const isMarkdownContent = (content: string): boolean => {
  return (
    content.includes("#") ||
    content.includes("**") ||
    content.includes("*") ||
    content.includes("```") ||
    (content.includes("[") && content.includes("](")) ||
    content.includes("- ") ||
    content.includes("1. ")
  );
};

// Detect content type for editing purposes
export const detectContentType = (
  document: Document
): "document" | "link" | "file" => {
  if (document.content.file && !document.content.message) {
    // Pure file document (file only, no message)
    return "file";
  } else if (
    document.content.url &&
    !document.content.message &&
    !document.content.file
  ) {
    // Pure URL document (URL only, no message or file)
    return "link";
  } else {
    // Message document (with or without file/URL attachments)
    return "document";
  }
};

// Format reply-to ID for navigation
export const formatReplyToId = (postId: number, documentId: number): string => {
  return `${postId}:${documentId}`;
};

// Parse reply-to ID back into components
export const parseReplyToId = (
  replyToId: string
): { postId: number; documentId: number } | null => {
  try {
    const parts = replyToId.split(":");
    if (parts.length !== 2) return null;

    const postId = parseInt(parts[0]);
    const documentId = parseInt(parts[1]);

    if (isNaN(postId) || isNaN(documentId)) return null;

    return { postId, documentId };
  } catch {
    return null;
  }
};
