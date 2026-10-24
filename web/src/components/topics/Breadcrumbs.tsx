import { Link } from "@tanstack/react-router";
import { Fragment } from "react";
import type { TopicRef } from "@/api/generated/TopicRef";

/** "Topics › Parent › Current": every part links except the current page. */
export function Breadcrumbs({ path }: { path: TopicRef[] }) {
  const sep = <span className="text-muted-foreground"> › </span>;
  return (
    <nav aria-label="Breadcrumb" className="text-sm">
      {path.length === 0 ? (
        <span aria-current="page">Topics</span>
      ) : (
        <Link to="/topics" className="underline-offset-2 hover:underline">
          Topics
        </Link>
      )}
      {path.map((t, i) => (
        <Fragment key={t.id}>
          {sep}
          {i === path.length - 1 ? (
            <span aria-current="page">{t.name}</span>
          ) : (
            <Link
              to="/topics/$topicId"
              params={{ topicId: String(t.id) }}
              className="underline-offset-2 hover:underline"
            >
              {t.name}
            </Link>
          )}
        </Fragment>
      ))}
    </nav>
  );
}
