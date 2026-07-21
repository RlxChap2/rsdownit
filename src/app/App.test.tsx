import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import App from "./App";

beforeEach(() => {
  window.localStorage.clear();
});

describe("rsdownit app shell", () => {
  it("renders the focused downloader workbench", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "rsdownit" })).toBeVisible();
    expect(
      screen.getByRole("heading", { name: "Download from almost anywhere." }),
    ).toBeVisible();
    expect(screen.getByLabelText("Media link")).toBeVisible();
    expect(screen.getByRole("button", { name: "Paste" })).toBeVisible();
    expect(screen.getByRole("group", { name: "Download mode" })).toBeVisible();
    expect(screen.getByRole("button", { name: /Video/ })).toBeVisible();
    expect(screen.getByRole("button", { name: /Audio/ })).toBeVisible();
    expect(screen.getByRole("button", { name: /Muted/ })).toBeVisible();
    expect(screen.getByText("No downloads yet.")).toBeVisible();
    expect((await screen.findAllByText("Downloads")).length).toBeGreaterThan(0);
    expect(await screen.findByText("Engine ready")).toBeVisible();
  });

  it("keeps network and session controls behind advanced settings", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(screen.queryByText("Self-hosted Cobalt API")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Settings" }));

    expect(screen.getByRole("dialog", { name: "Settings" })).toBeVisible();
    expect(screen.getByText("Download folder")).toBeVisible();
    expect(screen.getByRole("switch", { name: "Auto-start downloads" })).toBeVisible();
    expect(screen.getByRole("switch", { name: "Show advanced options" })).toBeVisible();
    expect(screen.queryByRole("switch", { name: "Community fallback servers" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("switch", { name: "Show advanced options" }));
    expect(await screen.findByText("Self-hosted Cobalt API")).toBeVisible();
    expect(screen.getByPlaceholderText("https://cobalt.example.com")).toBeVisible();
    expect(screen.getByRole("switch", { name: "Use browser session" })).toBeVisible();
    expect(screen.getByRole("switch", { name: "Community fallback servers" })).toBeVisible();
  });

  it("switches between light and dark themes", async () => {
    const user = userEvent.setup();
    render(<App />);

    const initialDark = document.documentElement.classList.contains("dark");
    const toggleName = initialDark ? "Switch to light mode" : "Switch to dark mode";
    await user.click(screen.getByRole("button", { name: toggleName }));
    expect(document.documentElement.classList.contains("dark")).toBe(!initialDark);
  });
});
