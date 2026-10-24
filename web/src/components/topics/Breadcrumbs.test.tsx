import { screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { renderWithApp } from "@/test/render";
import { Breadcrumbs } from "./Breadcrumbs";

it("renders_links_except_last", async () => {
  renderWithApp(
    <Breadcrumbs
      path={[
        { id: 1, name: "Electric circuits" },
        { id: 4, name: "Impedance" },
      ]}
    />,
  );
  expect(await screen.findByRole("link", { name: "Topics" })).toHaveAttribute("href", "/topics");
  expect(screen.getByRole("link", { name: "Electric circuits" })).toHaveAttribute("href", "/topics/1");
  expect(screen.queryByRole("link", { name: "Impedance" })).toBeNull();
  expect(screen.getByText("Impedance")).toHaveAttribute("aria-current", "page");
});
