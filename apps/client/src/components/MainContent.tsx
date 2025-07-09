import { useAppStore } from "../lib/store";
import { PodViewer } from "./PodViewer";
import { InboxView } from "./InboxView";
import { ChatView } from "./ChatView";

export function MainContent() {
  const { currentView } = useAppStore();

  switch (currentView) {
    case 'pods':
      return <PodViewer />;
    case 'inbox':
      return <InboxView />;
    case 'chats':
      return <ChatView />;
    default:
      return <PodViewer />;
  }
}