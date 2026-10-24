import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/questions/$questionId")({
  component: () => <h1 className="text-xl font-semibold">Question</h1>,
});
