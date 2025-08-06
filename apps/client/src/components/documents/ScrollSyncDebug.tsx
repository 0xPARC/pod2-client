import React, { useState, useEffect } from "react";
import { Button } from "../ui/button";

interface ScrollSyncDebugProps {
  scrollMapData: any;
  debugInfo: any;
  editAreaRef: React.RefObject<HTMLTextAreaElement | null>;
  viewAreaRef: React.RefObject<HTMLDivElement | null>;
}

export function ScrollSyncDebug({
  scrollMapData,
  debugInfo,
  editAreaRef,
  viewAreaRef
}: ScrollSyncDebugProps) {
  const [isVisible, setIsVisible] = useState(false);
  const [liveData, setLiveData] = useState<any>({});

  useEffect(() => {
    if (!isVisible) return;

    const interval = setInterval(() => {
      const editArea = editAreaRef.current;
      const viewArea = viewAreaRef.current;

      if (editArea && viewArea) {
        const lineHeight =
          parseFloat(getComputedStyle(editArea).lineHeight) || 20;
        const currentLineNo = Math.floor(editArea.scrollTop / lineHeight);
        const parts = viewArea.querySelectorAll(".part");

        setLiveData({
          editScrollTop: editArea.scrollTop,
          viewScrollTop: viewArea.scrollTop,
          editScrollHeight: editArea.scrollHeight,
          viewScrollHeight: viewArea.scrollHeight,
          editClientHeight: editArea.clientHeight,
          viewClientHeight: viewArea.clientHeight,
          lineHeight,
          currentLineNo,
          partCount: parts.length,
          scrollMapValue: scrollMapData?.scrollMap?.[currentLineNo] ?? "N/A",
          lineHeightMapValue:
            scrollMapData?.lineHeightMap?.[currentLineNo] ?? "N/A",
          // Debug values
          rawScrollCalc: `${editArea.scrollTop} / ${lineHeight} = ${editArea.scrollTop / lineHeight}`
        });
      }
    }, 100);

    return () => clearInterval(interval);
  }, [isVisible, editAreaRef, viewAreaRef, scrollMapData]);

  if (!isVisible) {
    return (
      <Button
        onClick={() => setIsVisible(true)}
        variant="outline"
        size="sm"
        className="fixed top-4 right-4 z-50 bg-background border"
      >
        Debug Sync
      </Button>
    );
  }

  return (
    <div className="fixed top-4 right-4 z-50 bg-background border rounded-lg p-4 max-w-sm text-xs space-y-2 shadow-lg">
      <div className="flex items-center justify-between">
        <h3 className="font-semibold">Scroll Sync Debug</h3>
        <Button
          onClick={() => setIsVisible(false)}
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0"
        >
          ×
        </Button>
      </div>

      <div className="space-y-1">
        <div>
          <strong>Content Lines:</strong>{" "}
          {debugInfo.content?.split("\n").length || 0}
        </div>
        <div>
          <strong>Has Scroll Map:</strong>{" "}
          {debugInfo.hasScrollMap ? "Yes" : "No"}
        </div>
        <div>
          <strong>Scroll Map Length:</strong> {debugInfo.scrollMapLength}
        </div>
        <div>
          <strong>Line Height Map Length:</strong>{" "}
          {debugInfo.lineHeightMapLength}
        </div>
        <div>
          <strong>Part Elements:</strong> {liveData.partCount || 0}
        </div>
      </div>

      <div className="border-t pt-2 space-y-1">
        <div>
          <strong>Edit Scroll:</strong> {liveData.editScrollTop}/
          {liveData.editScrollHeight}
        </div>
        <div>
          <strong>View Scroll:</strong> {liveData.viewScrollTop}/
          {liveData.viewScrollHeight}
        </div>
        <div>
          <strong>Line Height:</strong> {liveData.lineHeight}
        </div>
        <div>
          <strong>Current Line:</strong> {liveData.currentLineNo}
        </div>
        <div>
          <strong>Raw Calc:</strong> {liveData.rawScrollCalc}
        </div>
        <div>
          <strong>Scroll Map Value:</strong> {liveData.scrollMapValue}
        </div>
        <div>
          <strong>Line Height Map Value:</strong> {liveData.lineHeightMapValue}
        </div>
      </div>

      {scrollMapData?.scrollMap && (
        <div className="border-t pt-2">
          <div>
            <strong>Scroll Map (first 10):</strong>
          </div>
          <div className="text-xs font-mono max-h-20 overflow-y-auto">
            {scrollMapData.scrollMap
              .slice(0, 10)
              .map((value: number, idx: number) => (
                <div
                  key={idx}
                  className={
                    liveData.currentLineNo === idx
                      ? "bg-yellow-200 dark:bg-yellow-800"
                      : ""
                  }
                >
                  {idx}: {value}
                </div>
              ))}
            {scrollMapData.scrollMap.length > 10 && (
              <div>... ({scrollMapData.scrollMap.length - 10} more)</div>
            )}
          </div>
        </div>
      )}

      {scrollMapData?.lineHeightMap && (
        <div className="border-t pt-2">
          <div>
            <strong>Line Height Map (first 10):</strong>
          </div>
          <div className="text-xs font-mono max-h-20 overflow-y-auto">
            {scrollMapData.lineHeightMap
              .slice(0, 10)
              .map((value: number, idx: number) => (
                <div key={idx}>
                  {idx}: {value}
                </div>
              ))}
            {scrollMapData.lineHeightMap.length > 10 && (
              <div>... ({scrollMapData.lineHeightMap.length - 10} more)</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
