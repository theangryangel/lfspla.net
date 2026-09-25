import { getContext, setContext } from "svelte";
import type { TrackSummary, VehicleSummary } from "$lib/api.js";

/** Which half of a combination an opener wants changed. */
export type PickerMode = "combination" | "track" | "vehicle";

export interface PickerRequest {
  /** Changing one half alone needs both, so a request missing either browses. */
  mode?: PickerMode;
  track?: TrackSummary | null;
  vehicle?: VehicleSummary | null;
  /** The control that asked, so closing can hand focus back to it. */
  from?: HTMLElement | null;
}

export interface CombinationPicker {
  open: (request?: PickerRequest) => void;
}

const KEY = Symbol("combination-picker");

export function setCombinationPicker(picker: CombinationPicker) {
  setContext(KEY, picker);
}

/** The era's one picker dialog. Only components below the provider can ask. */
export function useCombinationPicker(): CombinationPicker {
  return getContext<CombinationPicker>(KEY);
}
