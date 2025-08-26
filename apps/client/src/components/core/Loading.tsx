import { ReactNode } from "react";
import { Card, CardContent } from "../ui/card";

export function Loading(): ReactNode {
  return (
    <div className="p-6 min-h-calc(100vh - var(--top-bar-height)) w-full">
      <div className="w-full">
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center justify-center py-12">
              <div className="text-center">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-4"></div>
                Loading...
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
