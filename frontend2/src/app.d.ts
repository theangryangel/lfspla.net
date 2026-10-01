// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
  namespace App {
    // interface Error {}
    // interface Locals {}
    interface PageData {
      /** Extra links after Upload hotlap; child loads can extend the parent's list. */
      hotlapActions?: { label: string; href: string }[];
    }
    // interface PageState {}
    // interface Platform {}
  }
}

export {};
