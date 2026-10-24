import { createFileRoute } from "@tanstack/react-router";
import { TopicView, validateTopicSearch } from "@/components/topics/TopicView";

export const Route = createFileRoute("/topics/")({
  validateSearch: validateTopicSearch,
  component: function TopicsRoot() {
    return <TopicView id={null} search={Route.useSearch()} />;
  },
});
