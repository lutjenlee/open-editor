import { describe, expect, it } from "vitest";
import { applyEditorCommand, CommandError, createEnvelope } from "./commands";
import { sampleProject } from "./sampleProject";
import { seconds, toSeconds } from "./time";

describe("editor command engine", () => {
  it("rejects stale revisions", () => {
    const envelope = createEnvelope(sampleProject, { type: "moveClip", trackId: "v1", clipId: "clip-1", timelineStart: seconds(1) });
    envelope.expectedProjectRevision -= 1;
    expect(() => applyEditorCommand(sampleProject, envelope)).toThrow(CommandError);
  });

  it("returns an exact inverse project snapshot", () => {
    const result = applyEditorCommand(sampleProject, createEnvelope(sampleProject, { type: "setVolume", trackId: "v1", clipId: "clip-1", volume: 0.4 }));
    expect(result.inversePatch.after).toEqual(sampleProject);
    expect(result.newProjectRevision).toBe(sampleProject.revision + 1);
  });

  it("splits speed-adjusted clips at the correct source position", () => {
    const project = structuredClone(sampleProject);
    const clip = project.sequences[0].tracks[0].clips[2];
    const timelineSplit = toSeconds(clip.timelineStart) + 2;
    const result = applyEditorCommand(project, createEnvelope(project, { type: "splitClip", trackId: "v1", clipId: clip.id, at: seconds(timelineSplit) }));
    const clips = result.forwardPatch.after.sequences[0].tracks[0].clips;
    const left = clips.find((item) => item.id === clip.id)!;
    const right = clips.find((item) => item.name === `${clip.name} B`)!;
    expect(toSeconds(left.sourceOut)).toBeCloseTo(toSeconds(clip.sourceIn) + 2 * clip.playbackRate);
    expect(right.sourceIn).toEqual(left.sourceOut);
  });
});
