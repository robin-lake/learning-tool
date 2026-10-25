import { createRootRoute, Link, Outlet } from "@tanstack/react-router";
import { TooltipProvider } from "@/components/ui/tooltip";

export const Route = createRootRoute({ component: RootLayout, notFoundComponent: NotFound });

const navLink =
  "text-sm text-muted-foreground hover:text-foreground [&.active]:text-foreground [&.active]:font-medium";

function RootLayout() {
  return (
    <TooltipProvider delayDuration={150}>
      <div className="min-h-screen">
        <header className="border-b">
          <nav className="mx-auto flex max-w-6xl items-center gap-6 px-4 py-3">
            <Link to="/" className="font-semibold">
              Learning tool
            </Link>
            <Link to="/" className={navLink} activeOptions={{ exact: true }}>
              Home
            </Link>
            <Link to="/topics" className={navLink}>
              Topics
            </Link>
            <Link to="/map" className={navLink}>
              Map
            </Link>
            <span className="text-sm text-muted-foreground/50" title="Coming soon" aria-disabled="true">
              Projects
            </span>
          </nav>
        </header>
        <main className="mx-auto max-w-6xl px-4 py-6">
          <Outlet />
        </main>
      </div>
    </TooltipProvider>
  );
}

function NotFound() {
  return (
    <div className="py-12">
      <h1 className="text-xl font-semibold">Page not found</h1>
      <p className="mt-2 text-muted-foreground">
        <Link to="/" className="underline">
          Go to the home page
        </Link>
      </p>
    </div>
  );
}
