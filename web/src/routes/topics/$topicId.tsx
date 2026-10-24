import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/topics/$topicId")({
  component: () => <h1 className="text-xl font-semibold">Topic</h1>,
});
