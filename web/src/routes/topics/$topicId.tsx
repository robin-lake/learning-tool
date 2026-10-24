import { createFileRoute } from "@tanstack/react-router";
import { TopicView, validateTopicSearch } from "@/components/topics/TopicView";

export const Route = createFileRoute("/topics/$topicId")({
  validateSearch: validateTopicSearch,
  component: function TopicRoute() {
    const { topicId } = Route.useParams();
    return <TopicView key={topicId} id={Number(topicId)} search={Route.useSearch()} />;
  },
});
