import { getContext, setContext, type Snippet } from "svelte";
import { SvelteMap } from "svelte/reactivity";

const KEY = Symbol("floating-panels");
type Panel = {
  order: number;
  placement: "bottom" | "top-right";
  content: Snippet;
};

export function setFloatingPanels() {
  return setContext(KEY, new SvelteMap<symbol, Panel>());
}

export function useFloatingPanels() {
  return getContext<SvelteMap<symbol, Panel>>(KEY);
}
