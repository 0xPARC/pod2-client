import React, { createContext, useContext, useCallback } from "react";
import { useAppStore } from "../store";
import { useKeyboardShortcuts } from "./useKeyboardShortcuts";
import { createShortcut } from "./types";
import type { KeyboardShortcut, KeyboardContext } from "./types";

interface KeyboardContextType {
  // Register a context with shortcuts
  registerContext: (context: KeyboardContext) => void;
  // Unregister a context
  unregisterContext: (contextName: string) => void;
  // Get all active shortcuts for debugging
  getActiveShortcuts: () => KeyboardShortcut[];
}

const KeyboardContext = createContext<KeyboardContextType | undefined>(
  undefined
);

export const useKeyboardContext = () => {
  const context = useContext(KeyboardContext);
  if (!context) {
    throw new Error(
      "useKeyboardContext must be used within a KeyboardProvider"
    );
  }
  return context;
};

export const KeyboardProvider: React.FC<{ children: React.ReactNode }> = ({
  children
}) => {
  const { setActiveApp } = useAppStore();

  // Global shortcuts that are always active
  const globalShortcuts: KeyboardShortcut[] = [
    // App switching shortcuts - keep priority to ensure they always work
    createShortcut(
      "1",
      () => setActiveApp("pod-collection"),
      "Switch to POD Collection",
      {
        cmd: true,
        priority: 100
      }
    ),
    createShortcut(
      "2",
      () => setActiveApp("documents"),
      "Switch to Documents",
      {
        cmd: true,
        priority: 100
      }
    ),
    createShortcut(
      "3",
      () => setActiveApp("pod-editor"),
      "Switch to POD Editor",
      {
        cmd: true,
        priority: 100
      }
    ),
    createShortcut(
      "4",
      () => setActiveApp("frogcrypto"),
      "Switch to FrogCrypto",
      {
        cmd: true,
        priority: 100
      }
    )
  ];

  // Register global shortcuts
  useKeyboardShortcuts(globalShortcuts, {
    enabled: true,
    context: "global"
  });

  // Context management functions (for future use if needed)
  const registerContext = useCallback((context: KeyboardContext) => {
    // For now, contexts are handled by individual components
    // This could be extended for more complex coordination
    console.debug(`Keyboard context registered: ${context.name}`);
  }, []);

  const unregisterContext = useCallback((contextName: string) => {
    console.debug(`Keyboard context unregistered: ${contextName}`);
  }, []);

  const getActiveShortcuts = useCallback((): KeyboardShortcut[] => {
    return globalShortcuts;
  }, []);

  const contextValue: KeyboardContextType = {
    registerContext,
    unregisterContext,
    getActiveShortcuts
  };

  return (
    <KeyboardContext.Provider value={contextValue}>
      {children}
    </KeyboardContext.Provider>
  );
};
