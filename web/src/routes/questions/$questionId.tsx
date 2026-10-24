import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { questionQuery } from "@/api/queries";
import { ErrorState } from "@/components/ErrorState";
import { AnswerRubric } from "@/components/questions/AnswerRubric";
import { AttemptHistory } from "@/components/questions/AttemptHistory";
import { QuestionHeader } from "@/components/questions/QuestionHeader";
import { RatingBox } from "@/components/questions/RatingBox";
import { ScheduleBox } from "@/components/questions/ScheduleBox";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export const Route = createFileRoute("/questions/$questionId")({ component: QuestionRoute });

function QuestionRoute() {
  const { questionId } = Route.useParams();
  const q = useQuery(questionQuery(Number(questionId)));
  if (q.isPending) return <Skeleton className="h-40" />;
  if (q.isError) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  const page = q.data;
  return (
    <div className="space-y-6">
      <QuestionHeader page={page} />
      <div className="grid gap-6 md:grid-cols-[1fr_18rem]">
        <AnswerRubric question={page.detail.question} citation={page.source_citation} />
        <Card className="py-4">
          <CardContent className="space-y-4 px-4">
            <ScheduleBox page={page} />
            <RatingBox questionId={page.detail.question.id} ratings={page.detail.ratings} />
          </CardContent>
        </Card>
      </div>
      <section className="space-y-2">
        <h2 className="font-medium">Attempt history</h2>
        <AttemptHistory reviews={page.detail.reviews} currentId={page.detail.question.id} />
      </section>
    </div>
  );
}
