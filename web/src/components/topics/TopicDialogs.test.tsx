import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { renderWithApp } from "@/test/render";
import { topic } from "./fixtures";
import { ArchiveTopicDialog, RenameTopicDialog } from "./TopicDialogs";

it("rename_requires_name", async () => {
  renderWithApp(<RenameTopicDialog topic={topic} />);
  await userEvent.click(await screen.findByRole("button", { name: "Rename" }));
  const name = screen.getByLabelText("Name");
  expect(name).toHaveValue("Impedance");
  await userEvent.clear(name);
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  expect(screen.getByText("A name is required.")).toBeInTheDocument();
});

it("archive_dialog_explains_history_is_kept", async () => {
  renderWithApp(<ArchiveTopicDialog topic={topic} />);
  await userEvent.click(await screen.findByRole("button", { name: "Archive" }));
  expect(screen.getByText(/stop being scheduled/)).toBeInTheDocument();
  expect(screen.getByText(/history is kept/)).toBeInTheDocument();
});
