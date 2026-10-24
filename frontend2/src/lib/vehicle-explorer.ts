export const vehicleClasses = [
  "Object",
  "Touring car",
  "Saloon car",
  "Buggy",
  "Formula",
  "GT",
  "Kart",
  "Bike",
  "Van",
  "Truck",
  "Formula 1",
  "Formula SAE",
  "Bus",
  "Prototype",
];
export const drives = ["None", "RWD", "FWD", "AWD"];
export const transmissions = [
  "None",
  "H-pattern",
  "Motorbike",
  "Sequential",
  "Sequential with cut",
  "Paddle shift",
  "Electric motor",
  "Centrifugal clutch",
];
const choices = (labels: string[]): [string, string][] =>
  labels.map((label, i) => [String(i), label]);
export const selects: {
  key: string;
  label: string;
  options: [string, string][];
}[] = [
  {
    key: "kind",
    label: "Standard / Mod",
    options: [
      ["standard", "Standard"],
      ["mod", "Mods"],
    ],
  },
  { key: "class", label: "Class", options: choices(vehicleClasses) },
  { key: "drive", label: "Drivetrain", options: choices(drives) },
  { key: "transmission", label: "Gearbox", options: choices(transmissions) },
  {
    key: "electric",
    label: "Engine",
    options: [
      ["false", "Combustion"],
      ["true", "Electric"],
    ],
  },
  {
    key: "approved",
    label: "Approval",
    options: [
      ["true", "Approved"],
      ["false", "Unapproved"],
    ],
  },
];
export const ranges: {
  key: string;
  label: string;
  unit: string;
  slider: { ceiling: number; step: number };
}[] = [
  {
    key: "power_kw",
    label: "Power",
    unit: "kW",
    slider: { ceiling: 600, step: 10 },
  },
  {
    key: "torque_nm",
    label: "Torque",
    unit: "Nm",
    slider: { ceiling: 1000, step: 25 },
  },
  {
    key: "mass_kg",
    label: "Mass",
    unit: "kg",
    slider: { ceiling: 2500, step: 25 },
  },
  {
    key: "displacement_cc",
    label: "Displacement",
    unit: "cc",
    slider: { ceiling: 7000, step: 100 },
  },
  {
    key: "power_weight",
    label: "Power / weight",
    unit: "W/kg",
    slider: { ceiling: 750, step: 10 },
  },
];
export const sorts: [string, string][] = [
  ["catalogue", "Catalogue order"],
  ["newest", "Newest published"],
  ["downloads", "Most downloaded"],
  ["rating", "Top rated"],
  ["power", "Most powerful"],
  ["mass", "Lightest"],
  ["power_weight", "Highest power / weight"],
];
export const filterKeys = [
  "q",
  "author",
  "sort",
  ...selects.map((s) => s.key),
  ...ranges.flatMap((r) => [`min_${r.key}`, `max_${r.key}`]),
];
