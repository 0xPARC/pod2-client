import { useState } from "react";

export interface UseSidebarStateReturn {
  leftSidebarCollapsed: boolean;
  rightSidebarCollapsed: boolean;
  setLeftSidebarCollapsed: (collapsed: boolean) => void;
  setRightSidebarCollapsed: (collapsed: boolean) => void;
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

  return {
    leftSidebarCollapsed,
    rightSidebarCollapsed,
    setLeftSidebarCollapsed,
    setRightSidebarCollapsed
  };
};
