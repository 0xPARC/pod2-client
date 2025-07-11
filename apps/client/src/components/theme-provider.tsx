import { createContext, useContext, useEffect, useState, ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

type Theme = "dark" | "light";

type ThemeProviderProps = {
  children: ReactNode;
};

type ThemeProviderState = {
  theme: Theme;
  setTheme: (theme: Theme) => void;
};

const initialState: ThemeProviderState = {
  theme: "light",
  setTheme: () => null
};

const ThemeProviderContext = createContext<ThemeProviderState>(initialState);

export function ThemeProvider({ children }: ThemeProviderProps) {
  const [theme, setTheme] = useState<Theme>("light");

  useEffect(() => {
    // Get initial theme from Tauri
    const getInitialTheme = async () => {
      try {
        const tauriTheme = await getCurrentWindow().theme();
        setTheme(tauriTheme || "light");
      } catch (error) {
        console.warn("Failed to get theme from Tauri:", error);
        setTheme("light");
      }
    };

    getInitialTheme();

    // Listen for theme changes
    const setupThemeListener = async () => {
      try {
        const unlisten = await getCurrentWindow().onThemeChanged(({ payload: newTheme }) => {
          setTheme(newTheme);
        });
        
        return unlisten;
      } catch (error) {
        console.warn("Failed to setup theme listener:", error);
        return () => {};
      }
    };

    let unlistenPromise = setupThemeListener();

    return () => {
      unlistenPromise.then(unlisten => unlisten());
    };
  }, []);

  // Update document class when theme changes
  useEffect(() => {
    const root = window.document.documentElement;
    const body = window.document.body;
    
    // Remove existing theme classes
    root.classList.remove("light", "dark");
    body.classList.remove("light", "dark");
    
    // Add current theme class to both root and body
    root.classList.add(theme);
    body.classList.add(theme);
    
    // For Tailwind CSS dark mode, ensure 'dark' class is on root
    if (theme === "dark") {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
  }, [theme]);

  const value = {
    theme,
    setTheme: (newTheme: Theme) => {
      setTheme(newTheme);
      // Note: We can't directly set the system theme from web context
      // This is mainly for manual overrides if needed
    }
  };

  return (
    <ThemeProviderContext.Provider value={value}>
      {children}
    </ThemeProviderContext.Provider>
  );
}

export const useTheme = () => {
  const context = useContext(ThemeProviderContext);

  if (context === undefined) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }

  return context;
};