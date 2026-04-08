import client from "./client";
import type { ChainFeatures } from "../types";

export interface FaucetConfig {
  enabled: boolean;
  amount_wei?: string;
  cooldown_minutes?: number;
}

export interface BrandingConfig {
  chain_name: string;
  logo_url?: string;
  logo_url_light?: string;
  logo_url_dark?: string;
  accent_color?: string;
  background_color_dark?: string;
  background_color_light?: string;
  success_color?: string;
  error_color?: string;
  features: ChainFeatures;
  faucet: FaucetConfig;
}

const defaultFeatures: ChainFeatures = { da_tracking: false };
const defaultFaucet: FaucetConfig = { enabled: false };

function normalizeFeatures(value: unknown): ChainFeatures {
  if (!value || typeof value !== "object") {
    return defaultFeatures;
  }

  const features = value as Partial<ChainFeatures>;
  return {
    da_tracking: features.da_tracking === true,
  };
}

function normalizeFaucet(value: unknown): FaucetConfig {
  if (!value || typeof value !== "object") {
    return defaultFaucet;
  }

  const faucet = value as Partial<FaucetConfig>;
  return {
    enabled: faucet.enabled === true,
    ...(typeof faucet.amount_wei === "string"
      ? { amount_wei: faucet.amount_wei }
      : {}),
    ...(typeof faucet.cooldown_minutes === "number"
      ? { cooldown_minutes: faucet.cooldown_minutes }
      : {}),
  };
}

export function normalizeBrandingConfig(
  config: Partial<BrandingConfig> & Pick<BrandingConfig, "chain_name">,
): BrandingConfig {
  return {
    ...config,
    features: normalizeFeatures(config.features),
    faucet: normalizeFaucet(config.faucet),
  };
}

export async function getConfig(): Promise<BrandingConfig> {
  const config = await client.get<
    Partial<BrandingConfig> & Pick<BrandingConfig, "chain_name">
  >("/config");
  return normalizeBrandingConfig(config);
}
