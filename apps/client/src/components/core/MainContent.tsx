import { useAppStore } from "../../lib/store";
import { DocumentsApp } from "../documents/DocumentsApp";
import { EditorView } from "../editor/EditorView";
import { FrogCrypto } from "../frogcrypto/FrogCrypto";
import { PodViewer } from "../pod-manager/PodViewer";

export function MainContent() {
  const { activeApp } = useAppStore();

  // Route based on active mini-app
  switch (activeApp) {
    case "documents":
      return <DocumentsApp />;
    case "pod-collection":
      return <PodViewer />;
    case "frogcrypto":
      return <FrogCrypto />;
    case "pod-editor":
      return <EditorView />;
    default:
      // Default to pod collection if no app is selected
      return <PodViewer />;
  }
}
