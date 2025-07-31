import { useState } from "react";
import { FeatureGate } from "../lib/features/config";
import { useAppStore } from "../lib/store";
import { DebugView } from "./DebugView";
import { FrogCrypto } from "./FrogCrypto";
import { PodViewer } from "./PodViewer";
import { DocumentsView } from "./documents/DocumentsView";
import { DraftsView } from "./documents/DraftsView";
import { PublishPage } from "./documents/PublishPage";
import { EditorView } from "./editor/EditorView";

export function MainContent() {
  const { currentView, previousView, setCurrentView } = useAppStore();
  const [editingDraftId, setEditingDraftId] = useState<string | null>(null); // UUID

  const handleEditDraft = (draftId: string) => {
    // UUID
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
          onPublishSuccess={(_documentId) => {
            setEditingDraftId(null);
            setCurrentView("drafts");
          }}
        />
      );
    case "drafts":
      return <DraftsView onEditDraft={handleEditDraft} />;
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
