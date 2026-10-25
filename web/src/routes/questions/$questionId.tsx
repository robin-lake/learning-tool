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
    <div className="space-y-10">
      <QuestionHeader page={page} />
      <div className="grid items-start gap-10 md:grid-cols-[minmax(0,42rem)_18rem] md:justify-between">
        <AnswerRubric question={page.detail.question} citation={page.source_citation} />
        <Card className="py-5">
          <CardContent className="px-5">
            <ScheduleBox page={page} />
            <div className="mt-6 border-t pt-6">
              <RatingBox questionId={page.detail.question.id} ratings={page.detail.ratings} />
            </div>
          </CardContent>
        </Card>
      </div>
      <section className="space-y-3">
        <h2 className="text-lg font-semibold tracking-tight">Attempt history</h2>
        <AttemptHistory reviews={page.detail.reviews} currentId={page.detail.question.id} />
      </section>
    </div>
  );
}
