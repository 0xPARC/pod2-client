import { useState } from "react";
import { Trash2, AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import { useAppStore } from "@/lib/store";
import type { PodInfo } from "@/lib/rpc";

interface DeletePodDialogProps {
  pod: PodInfo | null;
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
}

export function DeletePodDialog({
  pod,
  isOpen,
  onOpenChange
}: DeletePodDialogProps) {
  const [isDeleting, setIsDeleting] = useState(false);
  const deletePod = useAppStore((state) => state.deletePod);

  const handleDelete = async () => {
    if (!pod) return;

    try {
      setIsDeleting(true);
      await deletePod(pod.id, pod.space);
      toast.success("POD deleted successfully");
      onOpenChange(false);
    } catch (error) {
      console.error("Failed to delete POD:", error);
      toast.error("Failed to delete POD");
    } finally {
      setIsDeleting(false);
    }
  };

  const handleCancel = () => {
    if (!isDeleting) {
      onOpenChange(false);
    }
  };

  if (!pod) return null;

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[425px]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <AlertTriangle className="h-5 w-5 text-red-500" />
            Delete POD
          </DialogTitle>
          <DialogDescription>
            This action cannot be undone. The POD will be permanently deleted
            from your collection.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* POD Details */}
          <div className="rounded-lg border p-4 space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">POD ID</span>
              <code className="text-xs bg-gray-100 dark:bg-gray-800 px-2 py-1 rounded">
                {pod.id.slice(0, 12)}...
              </code>
            </div>

            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">Type</span>
              <Badge
                variant={pod.pod_type === "signed" ? "default" : "secondary"}
              >
                {pod.pod_type === "signed" ? "Signed" : "Main"}
              </Badge>
            </div>

            {pod.label && (
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">Label</span>
                <span className="text-sm text-gray-600 dark:text-gray-400">
                  {pod.label}
                </span>
              </div>
            )}

            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">Space</span>
              <span className="text-sm text-gray-600 dark:text-gray-400">
                {pod.space}
              </span>
            </div>

            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">Created</span>
              <span className="text-sm text-gray-600 dark:text-gray-400">
                {new Date(pod.created_at).toLocaleDateString()}
              </span>
            </div>
          </div>

          {/* Warning */}
          <div className="flex items-start gap-3 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
            <AlertTriangle className="h-4 w-4 text-red-500 flex-shrink-0 mt-0.5" />
            <div className="text-sm text-red-700 dark:text-red-300">
              <strong>Warning:</strong> This POD will be permanently deleted and
              cannot be recovered.
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={handleCancel}
            disabled={isDeleting}
          >
            Cancel
          </Button>
          <Button
            variant="destructive"
            onClick={handleDelete}
            disabled={isDeleting}
            className="gap-2"
          >
            {isDeleting ? (
              <>
                <div className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
                Deleting...
              </>
            ) : (
              <>
                <Trash2 className="h-4 w-4" />
                Delete POD
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
