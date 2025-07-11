import { useAppStore } from "../lib/store";
import { PodViewer } from "./PodViewer";
import { InboxView } from "./InboxView";
import { ChatView } from "./ChatView";
import { FrogCrypto } from "./FrogCrypto";
import { EditorView } from "./editor/EditorView";
import { FeatureGate } from "../lib/features/config";

export function MainContent() {
  const { currentView } = useAppStore();

  switch (currentView) {
    case "pods":
      return <PodViewer />;
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
    default:
      return <PodViewer />;
  }
}
