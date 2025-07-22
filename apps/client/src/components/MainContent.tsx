import { useAppStore } from "../lib/store";
import { PodViewer } from "./PodViewer";
import { InboxView } from "./InboxView";
import { ChatView } from "./ChatView";
import { FrogCrypto } from "./FrogCrypto";
import { DocumentsView } from "./documents/DocumentsView";
import { PublishPage } from "./documents/PublishPage";
import { EditorView } from "./editor/EditorView";
import { IdentitySettings } from "./identity/IdentitySettings";
import { FeatureGate } from "../lib/features/config";

export function MainContent() {
  const { currentView, appState } = useAppStore();

  // Extract username from Identity POD in the identity folder
  const getIdentityUsername = () => {
    try {
      // Find Identity POD in identity folder
      const identityPods = [
        ...appState.pod_lists.signed_pods,
        ...appState.pod_lists.main_pods,
        ...appState.pod_lists.rsa_intro_pods
      ].filter((pod) => pod.space === "identity");

      console.log("Identity pods:", identityPods);

      const identityPod = identityPods.find((pod) => {
        // Use label property which is the actual name field
        const podName = pod.label || "";
        return (
          podName === "Identity POD" ||
          podName.toLowerCase().includes("identity")
        );
      });

      console.log("Found identity pod:", identityPod);

      if (
        identityPod?.data?.pod_data_payload &&
        "entries" in identityPod?.data?.pod_data_payload
      ) {
        const usernameEntry =
          identityPod.data.pod_data_payload.entries.username;
        console.log("Username entry:", usernameEntry);
        if (usernameEntry) {
          return usernameEntry.toString();
        }
      }
    } catch (error) {
      console.error("Error getting identity username:", error);
    }

    return "unknown_user";
  };

  switch (currentView) {
    case "pods":
      return <PodViewer />;
    case "documents":
      return <DocumentsView />;
    case "publish":
      return <PublishPage />;
    case "identity":
      return <IdentitySettings currentUsername={getIdentityUsername()} />;
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
