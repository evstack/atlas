import { useBranding } from "./useBranding";

export default function useFeatures() {
  return useBranding().features;
}
