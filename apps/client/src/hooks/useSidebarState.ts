import { useState } from "react";

export interface UseSidebarStateReturn {
  leftSidebarCollapsed: boolean;
  rightSidebarCollapsed: boolean;
  setLeftSidebarCollapsed: (collapsed: boolean) => void;
  setRightSidebarCollapsed: (collapsed: boolean) => void;
  toggleLeftSidebar: () => void;
  toggleRightSidebar: () => void;
}

export const useSidebarState = (
  initialLeftCollapsed = false,
  initialRightCollapsed = true
): UseSidebarStateReturn => {
  const [leftSidebarCollapsed, setLeftSidebarCollapsed] =
    useState(initialLeftCollapsed);
  const [rightSidebarCollapsed, setRightSidebarCollapsed] = useState(
    initialRightCollapsed
  );

  const toggleLeftSidebar = () => {
    setLeftSidebarCollapsed(!leftSidebarCollapsed);
  };

  const toggleRightSidebar = () => {
    setRightSidebarCollapsed(!rightSidebarCollapsed);
  };

  return {
    leftSidebarCollapsed,
    rightSidebarCollapsed,
    setLeftSidebarCollapsed,
    setRightSidebarCollapsed,
    toggleLeftSidebar,
    toggleRightSidebar
  };
};
