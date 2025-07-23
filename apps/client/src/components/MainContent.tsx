import { useAppStore } from "../lib/store";
import { PodViewer } from "./PodViewer";
import { InboxView } from "./InboxView";
import { ChatView } from "./ChatView";
import { FrogCrypto } from "./FrogCrypto";
import { DocumentsView } from "./documents/DocumentsView";
import { PublishPage } from "./documents/PublishPage";
import { EditorView } from "./editor/EditorView";
import { DebugView } from "./DebugView";
import { FeatureGate } from "../lib/features/config";

export function MainContent() {
  const { currentView } = useAppStore();

  switch (currentView) {
    case "pods":
      return <PodViewer />;
    case "documents":
      return <DocumentsView />;
    case "publish":
      return <PublishPage />;
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
