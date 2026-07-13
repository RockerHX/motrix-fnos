export type AppMetricItem = {
  label: string;
  value: string | number;
  detail?: string;
  note?: string;
  tone?: "default" | "success" | "warning" | "error";
};
