export type UiResponsePayload =
  | { value: string }
  | { confirmed: boolean }
  | { cancelled: true };