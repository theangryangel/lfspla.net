import { getContext, setContext } from "svelte";
import type { PlayerSummary } from "./api.js";

export type CompareDriver = Pick<
  PlayerSummary,
  "lfs_username" | "display_name" | "country_code"
>;
const KEY = Symbol("compare");
const STORAGE_KEY = "lfspla.compare-drivers";
export class Comparison {
  drivers = $state<CompareDriver[]>([]);
  selected(username: string) {
    return this.drivers.some(
      (d) => d.lfs_username.toLowerCase() === username.toLowerCase(),
    );
  }
  seed(drivers: CompareDriver[], persist = true) {
    const unique = new Map<string, CompareDriver>();
    for (const driver of drivers) {
      if (
        driver &&
        typeof driver.lfs_username === "string" &&
        driver.lfs_username.trim() &&
        typeof driver.display_name === "string" &&
        (driver.country_code === null ||
          typeof driver.country_code === "string")
      ) {
        unique.set(driver.lfs_username.toLowerCase(), driver);
      }
    }
    const next = [...unique.values()].slice(0, 2);
    this.drivers = next;
    if (persist) {
      try {
        // Avoid reading reactive state when seed is called from an effect.
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      } catch {
        /* Storage is optional. */
      }
    }
  }
  restore = () => {
    try {
      const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
      this.seed(Array.isArray(value) ? value : [], false);
    } catch {
      this.seed([], false);
    }
  };
  storage = (event: StorageEvent) => {
    if (event.key === STORAGE_KEY || event.key === null) this.restore();
  };
  remove(username: string) {
    this.seed(
      this.drivers.filter(
        (d) => d.lfs_username.toLowerCase() !== username.toLowerCase(),
      ),
    );
  }
  toggle(driver: CompareDriver) {
    if (this.selected(driver.lfs_username)) this.remove(driver.lfs_username);
    else if (this.drivers.length < 2) this.seed([...this.drivers, driver]);
  }
}
export function setComparison() {
  return setContext(KEY, new Comparison());
}
export function useComparison() {
  return getContext<Comparison>(KEY);
}
