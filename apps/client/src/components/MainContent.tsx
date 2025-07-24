import { useState } from "react";
import { useAppStore } from "../lib/store";
import { PodViewer } from "./PodViewer";
import { InboxView } from "./InboxView";
import { ChatView } from "./ChatView";
import { FrogCrypto } from "./FrogCrypto";
import { DocumentsView } from "./documents/DocumentsView";
import { PublishPage } from "./documents/PublishPage";
import { DraftsView } from "./documents/DraftsView";
import { EditorView } from "./editor/EditorView";
import { DebugView } from "./DebugView";
import { FeatureGate } from "../lib/features/config";

export function MainContent() {
  const { currentView, previousView, setCurrentView } = useAppStore();
  const [editingDraftId, setEditingDraftId] = useState<number | null>(null);

  const handleEditDraft = (draftId: number) => {
    setEditingDraftId(draftId);
    setCurrentView("publish");
  };

  const handleBackFromPublish = () => {
    // Clear editing draft state
    setEditingDraftId(null);

    // If we came from drafts, go back to drafts
    if (previousView === "drafts") {
      setCurrentView("drafts");
    } else {
      // Default behavior - go to documents view
      setCurrentView("documents");
    }
  };

  switch (currentView) {
    case "pods":
      return <PodViewer />;
    case "documents":
      return <DocumentsView />;
    case "publish":
      return (
        <PublishPage
          onBack={handleBackFromPublish}
          editingDraftId={editingDraftId}
        />
      );
    case "drafts":
      return <DraftsView onEditDraft={handleEditDraft} />;
    case "inbox":
      return <InboxView />;
    case "chats":
      return <ChatView />;
    case "frogs":
      return <FrogCrypto />;
    case "editor":
      return (
        <FeatureGate feature="authoring">
          <EditorView />
        </FeatureGate>
      );
    case "debug":
      return <DebugView />;
    default:
      return <PodViewer />;
  }
}
