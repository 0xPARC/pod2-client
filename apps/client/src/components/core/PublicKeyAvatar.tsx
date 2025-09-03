import { useState, useEffect } from "react";
import { KeyRoundIcon } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";

interface PublicKeyAvatarProps {
  publicKey: string;
  size?: number;
  className?: string;
  showText?: boolean;
}

export function PublicKeyAvatar({
  publicKey,
  size = 16,
  className,
  showText = false
}: PublicKeyAvatarProps) {
  const [blockyImage, setBlockyImage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const generateBlockies = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const base64Data = await invoke<string>("generate_blockies", {
          publicKey
        });

        // Create data URL for display
        const dataUrl = `data:image/bmp;base64,${base64Data}`;
        setBlockyImage(dataUrl);
      } catch (err) {
        console.error("Failed to generate blockies:", err);
        setError(
          err instanceof Error ? err.message : "Failed to generate blockies"
        );
      } finally {
        setIsLoading(false);
      }
    };

    if (publicKey) {
      generateBlockies();
    }
  }, [publicKey]);

  const avatarElement = (
    <div
      className={cn(
        "flex items-center justify-center rounded-sm overflow-hidden bg-gray-100 dark:bg-gray-800",
        className
      )}
      style={{
        width: size,
        height: size,
        minWidth: size,
        minHeight: size
      }}
      title={publicKey}
    >
      {isLoading ? (
        <div className="animate-pulse bg-gray-300 dark:bg-gray-600 w-full h-full" />
      ) : error || !blockyImage ? (
        <KeyRoundIcon
          className="text-gray-400 dark:text-gray-500"
          size={Math.max(size * 0.6, 12)}
        />
      ) : (
        <img
          src={blockyImage}
          alt={`Avatar for ${publicKey}`}
          className="w-full h-full object-cover"
          style={{
            imageRendering: "pixelated" // Preserve pixelated look for blockies
          }}
        />
      )}
    </div>
  );

  if (showText) {
    return (
      <div className="flex items-center gap-2">
        {avatarElement}
        <span className="font-mono text-sm text-blue-600 dark:text-blue-400 truncate">
          {publicKey}
        </span>
      </div>
    );
  }

  return avatarElement;
}
