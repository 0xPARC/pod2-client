import React, { createContext, useContext, useEffect, useState } from "react";
import {
  getFeatureConfig as getFeatureConfigRpc,
  type FeatureConfig
} from "../rpc";

/**
 * React context for feature configuration
 */
const FeatureConfigContext = createContext<FeatureConfig | null>(null);

/**
 * Provider component that loads feature configuration from the backend
 */
export function FeatureConfigProvider({
  children
}: {
  children: React.ReactNode;
}) {
  const [config, setConfig] = useState<FeatureConfig | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    // Load configuration from backend on startup
    getFeatureConfigRpc()
      .then((loadedConfig) => {
        console.log("Loaded feature configuration:", loadedConfig);
        setConfig(loadedConfig);
      })
      .catch((error) => {
        console.error("Failed to load feature configuration:", error);
        // Set default configuration on error
        setConfig({
          pod_management: true,
          networking: true,
          authoring: true,
          integration: true
        });
      })
      .finally(() => {
        setLoading(false);
      });
  }, []);

  if (loading) {
    // Show loading state while configuration is being fetched
    return React.createElement(
      "div",
      {
        style: {
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          height: "100vh"
        }
      },
      React.createElement("div", null, "Loading application...")
    );
  }

  return React.createElement(
    FeatureConfigContext.Provider,
    { value: config },
    children
  );
}

/**
 * React hook for using feature configuration
 * This can be used in components to conditionally render based on features
 */
export function useFeatureConfig(): FeatureConfig {
  const config = useContext(FeatureConfigContext);

  if (!config) {
    throw new Error(
      "useFeatureConfig must be used within a FeatureConfigProvider"
    );
  }

  return config;
}

/**
 * Higher-order component for feature gating
 */
export function withFeatureGate<T extends object>(
  feature: keyof FeatureConfig,
  Component: React.ComponentType<T>
): React.ComponentType<T> {
  return function FeatureGatedComponent(props: T) {
    const config = useFeatureConfig();

    if (!config[feature]) {
      return null; // Don't render if feature is disabled
    }

    return React.createElement(Component, props);
  };
}

/**
 * Component for conditionally rendering children based on feature flags
 */
export function FeatureGate({
  feature,
  children,
  fallback = null
}: {
  feature: keyof FeatureConfig;
  children: React.ReactNode;
  fallback?: React.ReactNode;
}) {
  const config = useFeatureConfig();

  return config[feature]
    ? React.createElement(React.Fragment, null, children)
    : React.createElement(React.Fragment, null, fallback);
}

/**
 * Check if any features are enabled
 */
export function hasAnyFeaturesEnabled(config: FeatureConfig): boolean {
  return Object.values(config).some((enabled) => enabled);
}

/**
 * Get list of enabled feature names
 */
export function getEnabledFeatures(config: FeatureConfig): string[] {
  return Object.entries(config)
    .filter(([_, enabled]) => enabled)
    .map(([feature, _]) => feature);
}
