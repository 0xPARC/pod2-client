import {
  createContext,
  useContext,
  useState,
  useCallback,
  ReactNode,
  useLayoutEffect
} from "react";

export type TopBarPosition = "left" | "center" | "right";

interface TopBarSlots {
  left?: ReactNode;
  center?: ReactNode;
  right?: ReactNode;
}

interface TopBarContextType {
  slots: TopBarSlots;
  setSlot: (position: TopBarPosition, content: ReactNode) => void;
  clearSlot: (position: TopBarPosition) => void;
}

const TopBarContext = createContext<TopBarContextType | null>(null);

export function TopBarProvider({ children }: { children: ReactNode }) {
  const [slots, setSlotsState] = useState<TopBarSlots>({});

  const setSlot = useCallback(
    (position: TopBarPosition, content: ReactNode) => {
      setSlotsState((prev) => ({ ...prev, [position]: content }));
    },
    []
  );

  const clearSlot = useCallback((position: TopBarPosition) => {
    setSlotsState((prev) => ({ ...prev, [position]: undefined }));
  }, []);

  const value = {
    slots,
    setSlot,
    clearSlot
  };

  return (
    <TopBarContext.Provider value={value}>{children}</TopBarContext.Provider>
  );
}

export function useTopBar() {
  const context = useContext(TopBarContext);
  if (!context) {
    throw new Error("useTopBar must be used within a TopBarProvider");
  }
  return context;
}

// TopBar slot component - renders content into TopBar positions
export function TopBarSlot({
  position,
  children
}: {
  position: TopBarPosition;
  children: ReactNode;
}) {
  const { setSlot, clearSlot } = useTopBar();

  useLayoutEffect(() => {
    setSlot(position, children);
    return () => clearSlot(position);
  }, [position, children, setSlot, clearSlot]);

  return null; // Renders nothing in the local DOM
}
