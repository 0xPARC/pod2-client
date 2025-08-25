import { routeTree } from "@/routeTree.gen";
import type { QueryClient } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";

export interface RouterContext {
  queryClient: QueryClient;
}

export const router = createRouter({
  routeTree,
  context: {} as RouterContext
});

export type AppRouter = typeof router;
declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
    context: RouterContext;
  }
}
