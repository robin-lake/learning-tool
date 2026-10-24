import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import type { Topic } from "@/api/generated/Topic";
import { useCreateTopic, useUpdateTopic } from "@/api/queries";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";

const LEVELS = ["L0 Orientation", "L1 Working knowledge", "L2 Applied", "L3 Expert"];

function ServerError({ error }: { error: unknown }) {
  if (!error) return null;
  return <p className="text-sm text-destructive">{error instanceof Error ? error.message : String(error)}</p>;
}

export function RenameTopicDialog({ topic }: { topic: Topic }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState(topic.name);
  const [description, setDescription] = useState(topic.description);
  const update = useUpdateTopic();
  const empty = name.trim() === "";
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (o) {
          setName(topic.name);
          setDescription(topic.description);
          update.reset();
        }
      }}
    >
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          Rename
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Rename topic</DialogTitle>
          <DialogDescription>Questions and history stay attached to the topic.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label htmlFor="topic-name">Name</Label>
            <Input id="topic-name" value={name} onChange={(e) => setName(e.target.value)} />
            {empty && <p className="text-sm text-destructive">A name is required.</p>}
          </div>
          <div className="space-y-1">
            <Label htmlFor="topic-description">Description</Label>
            <Textarea
              id="topic-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          <ServerError error={update.error} />
        </div>
        <DialogFooter>
          <Button
            disabled={empty || update.isPending}
            onClick={() =>
              update.mutate(
                { id: topic.id, body: { name: name.trim(), description } },
                { onSuccess: () => setOpen(false) },
              )
            }
          >
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function ArchiveTopicDialog({ topic }: { topic: Topic }) {
  const update = useUpdateTopic();
  if (topic.status === "archived") {
    return (
      <Button
        variant="outline"
        size="sm"
        disabled={update.isPending}
        onClick={() => update.mutate({ id: topic.id, body: { status: "active" } })}
      >
        Unarchive
      </Button>
    );
  }
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="outline" size="sm">
          Archive
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Archive “{topic.name}”?</AlertDialogTitle>
          <AlertDialogDescription>
            Its questions, and those of its subtopics, will stop being scheduled. Nothing is deleted: the
            history is kept, and you can unarchive it any time.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction onClick={() => update.mutate({ id: topic.id, body: { status: "archived" } })}>
            Archive
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export function NewTopicDialog({ parent }: { parent: Topic | null }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [level, setLevel] = useState(String(parent?.target_level ?? 1));
  const [description, setDescription] = useState("");
  const create = useCreateTopic();
  const navigate = useNavigate();
  const empty = name.trim() === "";
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (o) {
          setName("");
          setDescription("");
          create.reset();
        }
      }}
    >
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          {parent ? "+ Subtopic" : "+ Topic"}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{parent ? `New subtopic of ${parent.name}` : "New topic"}</DialogTitle>
          <DialogDescription>Add questions to it later with /build-topic in Claude Code.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label htmlFor="new-topic-name">Name</Label>
            <Input id="new-topic-name" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="space-y-1">
            <Label htmlFor="new-topic-level">Target level</Label>
            <Select value={level} onValueChange={setLevel}>
              <SelectTrigger id="new-topic-level">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {LEVELS.map((l, i) => (
                  <SelectItem key={l} value={String(i)}>
                    {l}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label htmlFor="new-topic-description">Description</Label>
            <Textarea
              id="new-topic-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          <ServerError error={create.error} />
        </div>
        <DialogFooter>
          <Button
            disabled={empty || create.isPending}
            onClick={() =>
              create.mutate(
                { parent_id: parent?.id, name: name.trim(), target_level: Number(level), description },
                {
                  onSuccess: (t) => {
                    setOpen(false);
                    navigate({ to: "/topics/$topicId", params: { topicId: String(t.id) } });
                  },
                },
              )
            }
          >
            Create
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
