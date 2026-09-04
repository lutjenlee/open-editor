import type { RationalTime } from "../types/project";

export const TIMESCALE = 600;

export function seconds(value: number): RationalTime {
  return { value: Math.round(value * TIMESCALE), timescale: TIMESCALE };
}

export function toSeconds(time: RationalTime): number {
  if (time.timescale <= 0) throw new Error("timescale must be positive");
  return time.value / time.timescale;
}

export function formatTimecode(time: RationalTime, frameRate = 30): string {
  const totalFrames = Math.max(0, Math.round(toSeconds(time) * frameRate));
  const frames = totalFrames % frameRate;
  const totalSeconds = Math.floor(totalFrames / frameRate);
  const secs = totalSeconds % 60;
  const mins = Math.floor(totalSeconds / 60) % 60;
  const hours = Math.floor(totalSeconds / 3600);
  return [hours, mins, secs].map((part) => String(part).padStart(2, "0")).join(":") + `:${String(frames).padStart(2, "0")}`;
}
