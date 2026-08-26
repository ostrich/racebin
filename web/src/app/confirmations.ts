export type ConfirmationOptions = {
  title: string;
  message: string;
  confirmLabel?: string;
  dangerous?: boolean;
};

type ConfirmationPrompt = (options: ConfirmationOptions) => Promise<boolean>;
let prompt: ConfirmationPrompt | undefined;

export function setConfirmationPrompt(value: ConfirmationPrompt): void {
  prompt = value;
}

export function confirmAction(options: ConfirmationOptions): Promise<boolean> {
  if (!prompt) throw new Error("Confirmation dialog is not ready");
  return prompt(options);
}
