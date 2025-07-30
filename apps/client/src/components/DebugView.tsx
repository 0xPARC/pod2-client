import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  AlertTriangleIcon,
  CheckIcon,
  CopyIcon,
  DatabaseIcon,
  HardDriveIcon,
  RefreshCwIcon,
  SettingsIcon,
  TrashIcon
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface AppConfig {
  features: {
    pod_management: boolean;
    authoring: boolean;
    frogcrypto: boolean;
  };
  database: {
    path: string;
  };
  network: {
    document_server: string;
    identity_server: string;
    frogcrypto_server: string;
    timeout_seconds: number;
  };
  ui: {
    default_theme: string;
    default_window_width: number;
    default_window_height: number;
  };
  logging: {
    level: string;
    console_output: boolean;
  };
}

interface ExtendedAppConfig {
  config: AppConfig;
  config_file_path: string | null;
  database_full_path: string;
}

interface CacheStats {
  cache_path: string;
  total_size_bytes: number;
}

export function DebugView() {
  const [extendedConfig, setExtendedConfig] =
    useState<ExtendedAppConfig | null>(null);
  const [cacheStats, setCacheStats] = useState<CacheStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [cacheLoading, setCacheLoading] = useState(true);
  const [cacheClearLoading, setCacheClearLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cacheError, setCacheError] = useState<string | null>(null);

  // Load configuration on component mount
  useEffect(() => {
    loadConfig();
    loadCacheStats();
  }, []);

  const loadConfig = async () => {
    try {
      setLoading(true);
      setError(null);
      const extConfig = await invoke<ExtendedAppConfig>(
        "get_extended_app_config"
      );
      setExtendedConfig(extConfig);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load configuration"
      );
    } finally {
      setLoading(false);
    }
  };

  const loadCacheStats = async () => {
    try {
      setCacheLoading(true);
      setCacheError(null);
      const stats = await invoke<CacheStats>("get_cache_stats");
      setCacheStats(stats);
    } catch (err) {
      setCacheError(
        err instanceof Error ? err.message : "Failed to load cache statistics"
      );
    } finally {
      setCacheLoading(false);
    }
  };

  const clearPod2DiskCache = async () => {
    if (
      !confirm(
        "Are you sure you want to clear the POD2 disk cache? This action cannot be undone."
      )
    ) {
      return;
    }

    try {
      setCacheClearLoading(true);
      await invoke("clear_pod2_disk_cache");
      toast.success("POD2 disk cache cleared successfully!");
      // Reload cache stats to show updated size
      await loadCacheStats();
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to clear POD2 disk cache";
      toast.error(errorMessage);
    } finally {
      setCacheClearLoading(false);
    }
  };

  const handleCopyValue = async (value: string) => {
    try {
      await writeText(value);
      toast.success("Copied to clipboard!");
    } catch (error) {
      toast.error("Failed to copy to clipboard");
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 Bytes";

    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));

    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  };

  const ConfigSection = ({
    title,
    data,
    icon
  }: {
    title: string;
    data: Record<string, any>;
    icon: React.ReactNode;
  }) => (
    <Card className="mb-4">
      <CardHeader>
        <CardTitle className="text-lg flex items-center gap-2">
          {icon}
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="space-y-3">
          {Object.entries(data).map(([key, value]) => (
            <div
              key={key}
              className="flex items-center justify-between border-b pb-2"
            >
              <div className="flex-1">
                <div className="text-sm font-medium text-foreground">{key}</div>
                <div className="text-xs text-muted-foreground">
                  {typeof value === "boolean"
                    ? value
                      ? "✓ Enabled"
                      : "✗ Disabled"
                    : String(value)}
                </div>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => handleCopyValue(String(value))}
                className="ml-2"
              >
                <CopyIcon className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );

  if (loading) {
    return (
      <div className="p-6 w-full">
        <div className="w-full">
          <div className="flex items-center justify-center py-12">
            <div className="text-center">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-4"></div>
              Loading debug information...
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6 w-full">
        <div className="w-full">
          <Card className="border-destructive">
            <CardContent className="pt-6">
              <div className="flex items-center gap-2 text-destructive">
                <AlertTriangleIcon className="h-5 w-5" />
                <span>{error}</span>
              </div>
              <Button
                onClick={() => {
                  loadConfig();
                  loadCacheStats();
                }}
                variant="outline"
                className="mt-4"
              >
                <RefreshCwIcon className="h-4 w-4 mr-2" />
                Retry
              </Button>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 w-full overflow-y-auto">
      <div className="w-full max-w-4xl mx-auto">
        <div className="mb-6">
          <h1 className="text-3xl font-bold text-foreground mb-2">
            Debug Console
          </h1>
          <p className="text-muted-foreground">
            View current configuration and perform debug operations
          </p>
        </div>

        <div className="space-y-6">
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold">Current Configuration</h2>
            <Button
              onClick={() => {
                loadConfig();
                loadCacheStats();
              }}
              variant="outline"
              size="sm"
            >
              <RefreshCwIcon className="h-4 w-4 mr-2" />
              Refresh
            </Button>
          </div>

          {extendedConfig && (
            <div>
              {/* File Paths Section */}
              <ConfigSection
                title="File Paths"
                data={{
                  "Config File":
                    extendedConfig.config_file_path ||
                    "Using default configuration (no file)",
                  "Database File": extendedConfig.database_full_path
                }}
                icon={<DatabaseIcon className="h-5 w-5" />}
              />

              {/* Cache Information Section */}
              {cacheLoading ? (
                <Card className="mb-4">
                  <CardHeader>
                    <CardTitle className="text-lg flex items-center gap-2">
                      <HardDriveIcon className="h-5 w-5" />
                      Cache Information
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="flex items-center justify-center py-4">
                      <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary mr-2"></div>
                      Loading cache statistics...
                    </div>
                  </CardContent>
                </Card>
              ) : cacheError ? (
                <Card className="mb-4 border-destructive">
                  <CardHeader>
                    <CardTitle className="text-lg flex items-center gap-2">
                      <HardDriveIcon className="h-5 w-5" />
                      Cache Information
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="flex items-center gap-2 text-destructive">
                      <AlertTriangleIcon className="h-5 w-5" />
                      <span>{cacheError}</span>
                    </div>
                  </CardContent>
                </Card>
              ) : cacheStats ? (
                <Card className="mb-4">
                  <CardHeader>
                    <CardTitle className="text-lg flex items-center gap-2">
                      <HardDriveIcon className="h-5 w-5" />
                      POD2 Disk Cache
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="space-y-3">
                      <div className="flex items-center justify-between border-b pb-2">
                        <div className="flex-1">
                          <div className="text-sm font-medium text-foreground">
                            Cache Path
                          </div>
                          <div className="text-xs text-muted-foreground">
                            {cacheStats.cache_path}
                          </div>
                        </div>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleCopyValue(cacheStats.cache_path)}
                          className="ml-2"
                        >
                          <CopyIcon className="h-4 w-4" />
                        </Button>
                      </div>
                      <div className="flex items-center justify-between border-b pb-2">
                        <div className="flex-1">
                          <div className="text-sm font-medium text-foreground">
                            Cache Size
                          </div>
                          <div className="text-xs text-muted-foreground">
                            {formatBytes(cacheStats.total_size_bytes)}
                          </div>
                        </div>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            handleCopyValue(
                              formatBytes(cacheStats.total_size_bytes)
                            )
                          }
                          className="ml-2"
                        >
                          <CopyIcon className="h-4 w-4" />
                        </Button>
                      </div>
                      <div className="flex items-center justify-end pt-2">
                        <Button
                          variant="destructive"
                          size="sm"
                          onClick={clearPod2DiskCache}
                          disabled={cacheClearLoading}
                          className="flex items-center gap-2"
                        >
                          <TrashIcon className="h-4 w-4" />
                          {cacheClearLoading
                            ? "Clearing..."
                            : "Clear POD2 Cache"}
                        </Button>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              ) : null}

              <ConfigSection
                title="Network Settings"
                data={extendedConfig.config.network}
                icon={<SettingsIcon className="h-5 w-5" />}
              />
              <ConfigSection
                title="Feature Flags"
                data={extendedConfig.config.features}
                icon={<CheckIcon className="h-5 w-5" />}
              />
              <ConfigSection
                title="UI Settings"
                data={extendedConfig.config.ui}
                icon={<SettingsIcon className="h-5 w-5" />}
              />
              <ConfigSection
                title="Logging"
                data={extendedConfig.config.logging}
                icon={<SettingsIcon className="h-5 w-5" />}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
