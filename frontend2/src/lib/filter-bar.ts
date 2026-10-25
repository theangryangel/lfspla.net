export type FilterOption = {
  value: string;
  label: string;
  description?: string;
};
export type FilterDefinition = {
  key: string;
  label: string;
  placeholder?: string;
  options?: FilterOption[];
  loadOptions?: (query: string, signal: AbortSignal) => Promise<FilterOption[]>;
  range?: {
    unit: string;
    slider: { ceiling: number; step: number };
  };
};
export type FilterSort = {
  key: string;
  label: string;
  defaultValue: string;
  options: FilterOption[];
};
