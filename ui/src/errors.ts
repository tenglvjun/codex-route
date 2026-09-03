export function displayError(cause: unknown) {
  return cause instanceof Error ? cause.message : String(cause);
}
