import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { toast } from "sonner";
import { importPodFromJson } from "@/lib/features/pod-management";
import { Upload, FileText, X } from "lucide-react";

interface ImportPodDialogProps {
  trigger?: React.ReactNode;
}

export function ImportPodDialog({ trigger }: ImportPodDialogProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [label, setLabel] = useState("");
  const [podContent, setPodContent] = useState("");
  const [selectedFile, setSelectedFile] = useState<string | null>(null);

  const handleFileSelect = async () => {
    try {
      const file = await open({
        title: "Select POD File",
        filters: [
          {
            name: "POD Files",
            extensions: ["pod"],
          },
        ],
      });

      if (file && typeof file === "string") {
        setSelectedFile(file);
        // Read file content - Tauri doesn't provide file reading here,
        // we'll need to use the readTextFile API
        const { readTextFile } = await import("@tauri-apps/plugin-fs");
        const content = await readTextFile(file);
        setPodContent(content);
      }
    } catch (error) {
      toast.error("Failed to select file");
    }
  };

  const detectPodType = (content: string): string => {
    try {
      const pod = JSON.parse(content);
      
      // Try to detect POD type from structure
      if (pod.signature || pod.signer_public_key) {
        return "Signed";
      } else if (pod.proof || pod.statement) {
        return "Main";
      }
      
      // Default fallback
      return "Signed";
    } catch {
      return "Signed";
    }
  };

  const handleImport = async () => {
    if (!podContent.trim()) {
      toast.error("Please provide POD content");
      return;
    }

    try {
      setIsImporting(true);

      // Validate JSON format
      JSON.parse(podContent);

      const podType = detectPodType(podContent);
      
      await importPodFromJson(podContent, podType, label || undefined);

      toast.success("POD imported successfully");

      // Reset form
      setLabel("");
      setPodContent("");
      setSelectedFile(null);
      setIsOpen(false);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to import POD");
    } finally {
      setIsImporting(false);
    }
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const files = Array.from(e.dataTransfer.files);
    const podFile = files.find(f => f.name.endsWith('.pod'));

    if (podFile) {
      try {
        const content = await podFile.text();
        setPodContent(content);
        setSelectedFile(podFile.name);
      } catch (error) {
        toast.error("Failed to read dropped file");
      }
    } else {
      toast.error("Please drop a .pod file");
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogTrigger asChild>
        {trigger || (
          <Button variant="outline">
            <Upload className="mr-2 h-4 w-4" />
            Import POD
          </Button>
        )}
      </DialogTrigger>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>Import POD</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="label">Label (optional)</Label>
            <Input
              id="label"
              placeholder="Enter a friendly name for this POD"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
            />
          </div>

          <Tabs defaultValue="file">
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="file">File</TabsTrigger>
              <TabsTrigger value="text">Text</TabsTrigger>
            </TabsList>

            <TabsContent value="file" className="space-y-4">
              <div
                className="border-2 border-dashed border-muted-foreground/25 rounded-lg p-6 text-center space-y-4"
                onDragOver={handleDragOver}
                onDrop={handleDrop}
              >
                {selectedFile ? (
                  <div className="flex items-center justify-center space-x-2">
                    <FileText className="h-5 w-5 text-muted-foreground" />
                    <span className="text-sm">{selectedFile}</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        setSelectedFile(null);
                        setPodContent("");
                      }}
                    >
                      <X className="h-4 w-4" />
                    </Button>
                  </div>
                ) : (
                  <>
                    <Upload className="h-8 w-8 mx-auto text-muted-foreground" />
                    <div>
                      <p className="text-sm text-muted-foreground">
                        Drop a .pod file here, or
                      </p>
                      <Button variant="outline" onClick={handleFileSelect}>
                        Browse Files
                      </Button>
                    </div>
                  </>
                )}
              </div>
            </TabsContent>

            <TabsContent value="text" className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="pod-content">POD Content (JSON)</Label>
                <Textarea
                  id="pod-content"
                  placeholder="Paste POD JSON content here..."
                  value={podContent}
                  onChange={(e) => setPodContent(e.target.value)}
                  rows={8}
                  className="font-mono text-sm"
                />
              </div>
            </TabsContent>
          </Tabs>

          <div className="flex justify-end space-x-2">
            <Button variant="outline" onClick={() => setIsOpen(false)}>
              Cancel
            </Button>
            <Button onClick={handleImport} disabled={isImporting || !podContent.trim()}>
              {isImporting ? "Importing..." : "Import POD"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}