import { invoke } from "@tauri-apps/api/core";
import {
  EditIcon,
  FileIcon,
  FileTextIcon,
  LinkIcon,
  MessageSquareIcon,
  PlusIcon,
  SendIcon,
  Trash2Icon
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useAppStore } from "../../lib/store";
import { formatTimeAgo } from "../../lib/timeUtils";
import { Button } from "../ui/button";
import { Card, CardContent } from "../ui/card";

interface DraftInfo {
  id: number;
  title: string;
  content_type: string;
  message?: string;
  file_name?: string;
  file_content?: number[];
  file_mime_type?: string;
  url?: string;
  tags: string[];
  authors: string[];
  reply_to?: string;
  session_id?: string;
  created_at: string;
  updated_at: string;
}

interface DraftsViewProps {
  onEditDraft?: (draftId: number) => void;
}

export function DraftsView({ onEditDraft }: DraftsViewProps) {
  const [drafts, setDrafts] = useState<DraftInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [publishingDrafts, setPublishingDrafts] = useState<Set<number>>(
    new Set()
  );
  const { setCurrentView } = useAppStore();

  const loadDrafts = async () => {
    try {
      setLoading(true);
      const draftList = await invoke<DraftInfo[]>("list_drafts");
      setDrafts(draftList);
    } catch (error) {
      console.error("Failed to load drafts:", error);
      toast.error("Failed to load drafts");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadDrafts();
  }, []);

  const handleDeleteDraft = async (draftId: number) => {
    try {
      const success = await invoke<boolean>("delete_draft", { draftId });
      if (success) {
        setDrafts(drafts.filter((draft) => draft.id !== draftId));
        toast.success("Draft deleted");
      } else {
        toast.error("Failed to delete draft");
      }
    } catch (error) {
      console.error("Failed to delete draft:", error);
      toast.error("Failed to delete draft");
    }
  };

  const handlePublishDraft = async (draftId: number) => {
    try {
      setPublishingDrafts((prev) => new Set(prev).add(draftId));

      // Get server URL from configuration
      const networkConfig = await invoke<any>("get_config_section", {
        section: "network"
      });
      const serverUrl = networkConfig.document_server;

      const result = await invoke<{
        success: boolean;
        document_id?: number;
        error_message?: string;
      }>("publish_draft", {
        draftId,
        serverUrl
      });

      if (result.success) {
        toast.success("Draft published successfully!");
        // Remove the draft from the list since it was successfully published
        setDrafts(drafts.filter((draft) => draft.id !== draftId));
      } else {
        toast.error(result.error_message || "Failed to publish draft");
      }
    } catch (error) {
      console.error("Failed to publish draft:", error);
      toast.error("Failed to publish draft");
    } finally {
      setPublishingDrafts((prev) => {
        const newSet = new Set(prev);
        newSet.delete(draftId);
        return newSet;
      });
    }
  };

  const handleNewDraft = () => {
    setCurrentView("publish");
  };

  const getContentIcon = (contentType: string) => {
    switch (contentType) {
      case "message":
        return <MessageSquareIcon className="h-4 w-4" />;
      case "file":
        return <FileIcon className="h-4 w-4" />;
      case "url":
        return <LinkIcon className="h-4 w-4" />;
      default:
        return <FileTextIcon className="h-4 w-4" />;
    }
  };

  if (loading) {
    return (
      <div className="p-6 min-h-screen w-full overflow-y-auto">
        <div className="w-full max-w-4xl mx-auto">
          <div className="flex items-center justify-center py-12">
            <div className="animate-spin rounded-full h-8 w-8 border-b border-current"></div>
            <span className="ml-3">Loading drafts...</span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 min-h-screen w-full overflow-y-auto">
      <div className="w-full mx-auto">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-3xl font-bold">My Drafts</h1>
            <p className="text-muted-foreground mt-2">
              Manage your unpublished drafts
            </p>
          </div>
          <Button onClick={handleNewDraft} className="flex items-center gap-2">
            <PlusIcon className="h-4 w-4" />
            New Draft
          </Button>
        </div>

        {drafts.length === 0 ? (
          <Card>
            <CardContent className="flex flex-col items-center justify-center py-12">
              <FileTextIcon className="h-12 w-12 text-muted-foreground mb-4" />
              <h3 className="text-lg font-medium mb-2">No drafts yet</h3>
              <p className="text-muted-foreground text-center mb-4">
                Create your first draft to start writing documents
              </p>
              <Button
                onClick={handleNewDraft}
                className="flex items-center gap-2"
              >
                <PlusIcon className="h-4 w-4" />
                Create First Draft
              </Button>
            </CardContent>
          </Card>
        ) : (
          <div className="space-y-3">
            {drafts.map((draft) => (
              <Card
                key={draft.id}
                className="hover:shadow-md transition-shadow cursor-pointer"
                onClick={() => onEditDraft?.(draft.id)}
              >
                <CardContent className="px-4">
                  <div className="flex items-center justify-between">
                    <div className="flex-1 min-w-0 mr-4">
                      <div className="flex items-center gap-2 mb-1">
                        {getContentIcon(draft.content_type)}
                        <h3 className="font-medium text-base line-clamp-1">
                          {draft.title}
                        </h3>
                      </div>

                      <div className="flex items-center gap-2 text-sm text-muted-foreground">
                        <span>
                          Last edited {formatTimeAgo(draft.updated_at)}
                        </span>
                        {draft.reply_to && (
                          <>
                            <span>•</span>
                            <span>
                              Reply to #{draft.reply_to.split(":")[1]}
                            </span>
                          </>
                        )}
                        {draft.tags.length > 0 && (
                          <>
                            <span>•</span>
                            <span>
                              {draft.tags.length} tag
                              {draft.tags.length !== 1 ? "s" : ""}
                            </span>
                          </>
                        )}
                      </div>
                    </div>

                    <div
                      className="flex items-center gap-2"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => onEditDraft?.(draft.id)}
                        className="flex items-center gap-1"
                      >
                        <EditIcon className="h-3 w-3" />
                        Edit
                      </Button>
                      <Button
                        variant="default"
                        size="sm"
                        onClick={() => handlePublishDraft(draft.id)}
                        disabled={publishingDrafts.has(draft.id)}
                        className="flex items-center gap-1"
                      >
                        <SendIcon className="h-3 w-3" />
                        {publishingDrafts.has(draft.id)
                          ? "Publishing..."
                          : "Publish"}
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => handleDeleteDraft(draft.id)}
                        className="flex items-center gap-1 text-destructive hover:text-destructive"
                      >
                        <Trash2Icon className="h-3 w-3" />
                        Delete
                      </Button>
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
