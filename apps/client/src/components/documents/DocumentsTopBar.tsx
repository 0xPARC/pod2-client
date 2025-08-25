import {
  ArrowLeftIcon,
  ArrowRightIcon,
  LinkIcon,
  ShareIcon
} from "lucide-react";
import { useRouter } from "@tanstack/react-router";
// Deep-link URL generation moved inline to avoid store dependencies
import { createShortcut } from "@/lib/keyboard/types";
import { useKeyboardShortcuts } from "@/lib/keyboard/useKeyboardShortcuts";
import { TopBarSlot } from "@/components/core/TopBarContext";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";

interface DocumentsTopBarProps {
  title: string;
  prefix?: string;
  onNewDocument?: () => void;
}

export function DocumentsTopBar({
  title,
  prefix,
  onNewDocument
}: DocumentsTopBarProps) {
  const router = useRouter();

  // Use browser history for back/forward
  const canGoBack = window.history.length > 1;
  // Note: Browser doesn't expose forward history length, so we can't reliably disable the button

  const handleGoBack = () => {
    router.history.go(-1);
  };

  const handleGoForward = () => {
    router.history.go(1);
  };

  const handleCopyLink = () => {
    const currentPath = window.location.pathname;
    const currentUrl = `${window.location.origin}${currentPath}`;

    // Copy the current URL (deep-link functionality could be added later)
    navigator.clipboard.writeText(currentUrl);
  };

  // Documents app keyboard shortcuts
  const documentsShortcuts = [
    // New document
    ...(onNewDocument
      ? [
          createShortcut("n", onNewDocument, "New Document", {
            cmd: true
          })
        ]
      : []),
    // Back navigation
    createShortcut(
      "[",
      () => {
        if (canGoBack) {
          handleGoBack();
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
        handleGoForward();
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

  return (
    <TopBarSlot position="left">
      <div className="flex items-center gap-1 mr-4">
        <Button
          variant="ghost"
          size="sm"
          disabled={!canGoBack}
          onClick={handleGoBack}
          title="Go back"
        >
          <ArrowLeftIcon className="w-4 h-4" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={handleGoForward}
          title="Go forward"
        >
          <ArrowRightIcon className="w-4 h-4" />
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm">
              <ShareIcon className="w-4 h-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onClick={handleCopyLink}>
              <LinkIcon className="w-4 h-4" />
              Copy link
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <h1>
          {prefix && (
            <span className="font-normal text-muted-foreground mr-1">
              {prefix}
            </span>
          )}
          {title}
        </h1>
      </div>
    </TopBarSlot>
  );
}
