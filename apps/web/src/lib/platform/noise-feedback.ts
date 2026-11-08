/**
 * Whether the child's audio plays for a noise alert. Mirrors `FeedbackPlans.forNoise` on Android:
 * snoozed noise is visual only, and the discreet alert style withholds the audio until the parent
 * listens (the model unmutes on Listen). Presentation only; the core decides nothing here.
 */
export function noisePlaysAudio(snoozed: boolean, discreet: boolean): boolean {
  return !snoozed && !discreet;
}
