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
export const getFileFilters = (mimeType: string) => {
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
export const ensureFileExtension = (
  filename: string,
  mimeType: string
): string => {
  const ext = getMimeTypeExtension(mimeType);
  if (ext === "bin") return filename;

  const hasExtension =
    filename.includes(".") && filename.split(".").pop()?.toLowerCase() === ext;
  return hasExtension ? filename : `${filename}.${ext}`;
};

// Check if file type is an image
export const isImageFile = (mimeType: string): boolean => {
  return mimeType.startsWith("image/");
};

// Check if file type is markdown
export const isMarkdownFile = (mimeType: string, filename: string): boolean => {
  return (
    mimeType === "text/markdown" ||
    filename.toLowerCase().endsWith(".md") ||
    filename.toLowerCase().endsWith(".markdown")
  );
};

// Check if file type is text
export const isTextFile = (mimeType: string): boolean => {
  return mimeType.startsWith("text/");
};

// Convert file content byte array to string
export const fileContentToString = (content: number[]): string => {
  return String.fromCharCode(...content);
};

// Convert file content to base64 data URL
export const fileContentToDataUrl = (
  content: number[],
  mimeType: string
): string => {
  return `data:${mimeType};base64,${btoa(String.fromCharCode(...content))}`;
};
