import { VehicleClass, Drivetrain, Transmission } from "@lfsplanet/sdk/types";

export const vehicleClasses = {
  [VehicleClass.Object]: "Object",
  [VehicleClass.TouringCar]: "Touring car",
  [VehicleClass.SaloonCar]: "Saloon car",
  [VehicleClass.Buggy]: "Buggy",
  [VehicleClass.Formula]: "Formula",
  [VehicleClass.Gt]: "GT",
  [VehicleClass.Kart]: "Kart",
  [VehicleClass.Bike]: "Bike",
  [VehicleClass.Van]: "Van",
  [VehicleClass.Truck]: "Truck",
  [VehicleClass.Formula1]: "Formula 1",
  [VehicleClass.FormulaSae]: "Formula SAE",
  [VehicleClass.Bus]: "Bus",
  [VehicleClass.Prototype]: "Prototype",
  [VehicleClass.Reserved14]: "Class 14",
} satisfies Record<VehicleClass, string>;

export const drives = {
  [Drivetrain.None]: "None",
  [Drivetrain.RearWheelDrive]: "RWD",
  [Drivetrain.FrontWheelDrive]: "FWD",
  [Drivetrain.AllWheelDrive]: "AWD",
} satisfies Record<Drivetrain, string>;

export const transmissions = {
  [Transmission.None]: "None",
  [Transmission.HPattern]: "H-pattern",
  [Transmission.Motorbike]: "Motorbike",
  [Transmission.Sequential]: "Sequential",
  [Transmission.SequentialWithCut]: "Sequential with cut",
  [Transmission.Paddle]: "Paddle shift",
  [Transmission.Electric]: "Electric motor",
  [Transmission.CentrifugalClutch]: "Centrifugal clutch",
} satisfies Record<Transmission, string>;

const choices = (labels: Record<string, string>): [string, string][] =>
  Object.entries(labels);
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
