import { ArrowLeftIcon, ArrowRightIcon } from "lucide-react";
import { createShortcut } from "../../lib/keyboard/types";
import { useKeyboardShortcuts } from "../../lib/keyboard/useKeyboardShortcuts";
import { useDocuments } from "../../lib/store";
import { DebugView } from "../DebugView";
import { TopBarSlot } from "../TopBarContext";
import { Button } from "../ui/button";
import { DocumentDetailView } from "./DocumentDetailView";
import { DocumentsView } from "./DocumentsView";
import { DraftsView } from "./DraftsView";
import { PublishPage } from "./PublishPage";

// Helper function to get route title data for breadcrumb
const getRouteTitle = (
  route: any
): { text: string; hasPrefix?: boolean; prefix?: string } => {
  switch (route?.type) {
    case "documents-list":
      return { text: "Documents" };
    case "document-detail":
      // Use document title if available, fallback to document number
      if (route.title) {
        return { text: route.title, hasPrefix: true, prefix: "Document:" };
      }
      return { text: `Document #${route.id}` };
    case "drafts":
      return { text: "Drafts" };
    case "publish":
      if (route.editingDraftId) {
        return { text: "Edit Draft" };
      }
      // Show content type in title
      switch (route.contentType) {
        case "link":
          return { text: "New Link" };
        case "file":
          return { text: "New File" };
        case "document":
        default:
          return { text: "New Document" };
      }
    case "debug":
      return { text: "Debug" };
    default:
      return { text: "Documents" };
  }
};

// Main Documents app component
export function DocumentsApp() {
  const {
    browsingHistory,
    navigateToDocument,
    navigateToPublish,
    goBack,
    goForward
  } = useDocuments();

  const currentRoute = browsingHistory.stack[browsingHistory.currentIndex] || {
    type: "documents-list"
  };

  const canGoBack = browsingHistory.currentIndex > 0;
  const canGoForward =
    browsingHistory.currentIndex < browsingHistory.stack.length - 1;
  const titleData = getRouteTitle(currentRoute);

  // Documents app keyboard shortcuts
  const documentsShortcuts = [
    // New document
    createShortcut("n", () => navigateToPublish(), "New Document", {
      cmd: true
    }),
    // Back navigation
    createShortcut(
      "[",
      () => {
        if (browsingHistory.currentIndex > 0) {
          goBack();
        }
      },
      "Go Back",
      {
        cmd: true
      }
    ),
    // Forward navigation
    createShortcut(
      "]",
      () => {
        if (browsingHistory.currentIndex < browsingHistory.stack.length - 1) {
          goForward();
        }
      },
      "Go Forward",
      {
        cmd: true
      }
    )
  ];

  useKeyboardShortcuts(documentsShortcuts, {
    enabled: true,
    context: "documents"
  });

  const renderRoute = () => {
    switch (currentRoute.type) {
      case "documents-list":
        return <DocumentsView />;
      case "document-detail":
        if (currentRoute.id) {
          return (
            <DocumentDetailView
              documentId={currentRoute.id}
              onNavigateToDocument={navigateToDocument}
            />
          );
        }
        return <DocumentsView />;
      case "drafts":
        return (
          <DraftsView onEditDraft={(draftId) => navigateToPublish(draftId)} />
        );
      case "publish":
        return (
          <PublishPage
            editingDraftId={currentRoute.editingDraftId || null}
            contentType={currentRoute.contentType || "document"}
            replyTo={currentRoute.replyTo}
            onPublishSuccess={() => {
              // Navigation is now handled by the top-level back/forward buttons
            }}
          />
        );
      case "debug":
        return <DebugView />;
      default:
        return <DocumentsView />;
    }
  };

  return (
    <div className="flex flex-col h-full w-full">
      {/* TopBar Content */}
      <TopBarSlot position="left">
        <div className="flex items-center gap-1 mr-4">
          <Button
            variant="ghost"
            size="sm"
            disabled={!canGoBack}
            onClick={goBack}
            title="Go back"
          >
            <ArrowLeftIcon className="w-4 h-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={!canGoForward}
            onClick={goForward}
            title="Go forward"
          >
            <ArrowRightIcon className="w-4 h-4" />
          </Button>
          <h1>
            {titleData.hasPrefix && titleData.prefix && (
              <span className="font-normal text-muted-foreground mr-1">
                {titleData.prefix}
              </span>
            )}
            {titleData.text}
          </h1>
        </div>
      </TopBarSlot>

      <div className="flex-1 overflow-auto">{renderRoute()}</div>
    </div>
  );
}
