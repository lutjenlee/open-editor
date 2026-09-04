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

  it("keeps provider commands inside the user-approved media scope", () => {
    const envelope = createEnvelope(sampleProject, { type: "removeMedia", assetId: "asset-6" }, "codex");
    expect(() => applyEditorCommand(sampleProject, envelope)).toThrow(/approved media scope/);
  });

  it("applies caption, audio, transform, replacement, and transition commands", () => {
    let project = structuredClone(sampleProject);
    const run = (payload: Parameters<typeof createEnvelope>[1]) => {
      const result = applyEditorCommand(project, createEnvelope(project, payload));
      expect(result.inversePatch.after).toEqual(project);
      project = result.forwardPatch.after;
    };
    run({ type: "fadeAudio", trackId: "v1", clipId: "clip-1", fadeIn: seconds(0.2), fadeOut: seconds(0.3) });
    run({ type: "duckAudio", trackId: "v1", clipId: "clip-1", enabled: true });
    run({ type: "cropClip", trackId: "v1", clipId: "clip-1", transform: { x: 12, y: -8, scale: 1.2, rotation: 2, opacity: 0.9 } });
    run({ type: "replaceClip", trackId: "v1", clipId: "clip-1", assetId: "asset-2" });
    run({ type: "addCaption", trackId: "c1", start: seconds(10), end: seconds(12), text: "A new caption" });
    run({ type: "addTransition", fromClipId: "clip-1", toClipId: "clip-2", kind: "crossDissolve", duration: seconds(0.4) });
    const sequence = project.sequences[0];
    expect(sequence.captions.at(-1)?.text).toBe("A new caption");
    expect(sequence.transitions).toHaveLength(1);
    const clip = sequence.tracks[0].clips[0];
    expect(clip.audio.ducking).toBe(true);
    expect(clip.transform.scale).toBe(1.2);
    expect(clip.assetId).toBe("asset-2");
  });

  it("duplicates a sequence without reusing mutable entity identifiers", () => {
    const result = applyEditorCommand(sampleProject, createEnvelope(sampleProject, {
      type: "duplicateSequence", sequenceId: sampleProject.activeSequenceId, name: "Alternative",
    }));
    const original = result.forwardPatch.after.sequences[0];
    const alternative = result.forwardPatch.after.sequences[1];
    expect(alternative.name).toBe("Alternative");
    expect(result.forwardPatch.after.activeSequenceId).toBe(alternative.id);
    expect(alternative.id).not.toBe(original.id);
    expect(alternative.tracks[0].id).not.toBe(original.tracks[0].id);
    expect(alternative.tracks[0].clips[0].id).not.toBe(original.tracks[0].clips[0].id);
  });
});
