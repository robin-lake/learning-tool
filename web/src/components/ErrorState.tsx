import { AlertCircle } from "lucide-react";
import { ApiError } from "@/api/client";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

export function ErrorState({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  const network = error instanceof ApiError && error.kind === "network";
  const message = error instanceof Error ? error.message : String(error);
  return (
    <Alert variant="destructive" className="my-4">
      <AlertCircle />
      <AlertTitle>{network ? "Couldn't reach the learning tool" : "Something went wrong"}</AlertTitle>
      <AlertDescription>
        <p>{network ? "Is `learning-tool serve` still running?" : message}</p>
        <Button variant="outline" size="sm" className="mt-2" onClick={onRetry}>
          Retry
        </Button>
      </AlertDescription>
    </Alert>
  );
}
