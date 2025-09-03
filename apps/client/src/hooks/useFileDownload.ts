import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { toast } from "sonner";
import { DocumentFile } from "../lib/documentApi";
import { getFileFilters, ensureFileExtension } from "../lib/fileUtils";

export interface UseFileDownloadReturn {
  downloadingFiles: Set<string>;
  handleDownloadFile: (file: DocumentFile) => Promise<void>;
}

export const useFileDownload = (): UseFileDownloadReturn => {
  const [downloadingFiles, setDownloadingFiles] = useState<Set<string>>(
    new Set()
  );

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

  return {
    downloadingFiles,
    handleDownloadFile
  };
};
