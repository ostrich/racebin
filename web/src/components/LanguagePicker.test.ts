import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import LanguagePicker from "./LanguagePicker.svelte";

describe("LanguagePicker", () => {
  it("filters aliases and selects the canonical language", async () => {
    render(LanguagePicker, { value: "auto" });
    const input = screen.getByRole("combobox");
    await fireEvent.focus(input);
    await fireEvent.input(input, { target: { value: "js" } });
    await fireEvent.click(screen.getByRole("option", { name: /JavaScript/ }));
    expect(input).toHaveValue("javascript");
  });

  it("is unavailable for non-text pastes", () => {
    render(LanguagePicker, { value: "plaintext", disabled: true });
    expect(screen.getByRole("combobox")).toBeDisabled();
  });

  it.each([{ isComposing: true }, { keyCode: 229 }])(
    "does not select an option while confirming IME composition (%j)",
    async (composition) => {
      render(LanguagePicker, { value: "plaintext" });
      const input = screen.getByRole("combobox");
      await fireEvent.focus(input);
      await fireEvent.input(input, { target: { value: "js" } });
      await fireEvent.keyDown(input, { key: "ArrowDown" });
      await fireEvent.keyDown(input, { key: "Enter", ...composition });
      expect(screen.getByRole("listbox")).toBeVisible();
      await fireEvent.keyDown(input, { key: "Enter" });
      expect(input).toHaveValue("javascript");
    }
  );
});
