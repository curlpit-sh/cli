import { Outlet, RouterProvider, createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { StrictMode } from "react";
import type { JSX } from "react";

import { Playground } from "./components/Playground";

const rootRoute = createRootRoute({
  component: () => (
    <StrictMode>
      <Outlet />
    </StrictMode>
  ),
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: Playground,
});

const routeTree = rootRoute.addChildren([indexRoute]);

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

export function Router(): JSX.Element {
  return <RouterProvider router={router} />;
}
