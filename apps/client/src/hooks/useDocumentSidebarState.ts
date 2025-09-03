import { useState } from "react";

export interface UseDocumentSidebarStateReturn {
  leftSidebarCollapsed: boolean;
  rightSidebarCollapsed: boolean;
  setLeftSidebarCollapsed: (collapsed: boolean) => void;
  setRightSidebarCollapsed: (collapsed: boolean) => void;
}

export const useDocumentSidebarState = (
  initialLeftCollapsed = false,
  initialRightCollapsed = true
): UseDocumentSidebarStateReturn => {
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
