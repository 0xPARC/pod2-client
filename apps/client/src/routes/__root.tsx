import { AppSidebar } from "@/components/core/AppSidebar";
import { TopBar } from "@/components/core/TopBar";
import { TopBarProvider, TopBarSlot } from "@/components/core/TopBarContext";
import { SidebarProvider, useSidebar } from "@/components/ui/sidebar";
import { createShortcut } from "@/lib/keyboard/types";
import { useKeyboardShortcuts } from "@/lib/keyboard/useKeyboardShortcuts";
import { RouterContext } from "@/lib/router";
import {
  createRootRouteWithContext,
  Outlet,
  useMatches
} from "@tanstack/react-router";
import React from "react";

function GlobalBreadcrumbs() {
  // We'll read staticData.breadcrumb from matches if provided
  try {
    const matches = useMatches();
    const parts = matches
      .map((m: any) =>
        typeof m.staticData?.breadcrumb === "function"
          ? m.staticData.breadcrumb(m)
          : m.staticData?.breadcrumb
      )
      .filter(Boolean) as React.ReactNode[];

    if (!parts.length) return null;

    return (
      <div className="flex items-center gap-2">
        {parts.map((node, i) => (
          <React.Fragment key={i}>
            <span
              className={
                i === parts.length - 1
                  ? "font-semibold"
                  : "text-muted-foreground"
              }
            >
              {node}
            </span>
            {i < parts.length - 1 && (
              <span className="text-muted-foreground">/</span>
            )}
          </React.Fragment>
        ))}
      </div>
    );
  } catch (error) {
    // Router context not available yet, render nothing
    console.log("Router context not ready for breadcrumbs");
    return null;
  }
}

// Component that handles global keyboard shortcuts within the sidebar context
function GlobalKeyboardShortcuts() {
  const { toggleSidebar } = useSidebar();

  const globalShortcuts = [
    createShortcut("b", () => toggleSidebar(), "Toggle Sidebar", {
      cmd: true
    })
  ];

  useKeyboardShortcuts(globalShortcuts, {
    enabled: true,
    context: "global"
  });

  return null;
}

const RootComponent = function Root() {
  return (
    <>
      <SidebarProvider className="h-screen">
        <TopBarProvider>
          <GlobalKeyboardShortcuts />
          <TopBar />
          <TopBarSlot position="left">
            <GlobalBreadcrumbs />
          </TopBarSlot>
          <AppSidebar />
          <div className="pt-(--top-bar-height) w-full h-full overflow-scroll">
            <Outlet />
          </div>
        </TopBarProvider>
      </SidebarProvider>
    </>
  );
};

export const Route = createRootRouteWithContext<RouterContext>()({
  component: RootComponent
});
