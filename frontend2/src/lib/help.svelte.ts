import { getContext, setContext, tick } from "svelte";

export type HelpTopic = "teams" | "hotlaps";
const KEY = Symbol("lfsplanet.help");

class Help {
  open = $state(false);
  trigger: HTMLElement | null = null;

  toggle = (trigger: HTMLElement) => {
    this.trigger = trigger;
    this.open = !this.open;
  };

  close = async () => {
    this.open = false;
    await tick();
    const trigger = this.trigger?.isConnected
      ? this.trigger
      : document.querySelector<HTMLElement>("[data-help-trigger]");
    trigger?.focus();
  };
}

export function setHelp() {
  return setContext(KEY, new Help());
}

export function useHelp() {
  return getContext<Help>(KEY);
}
